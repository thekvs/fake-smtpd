use failure::Error;
use rand::prelude::*;
use regex::Regex;
use std::io::Read;

mod command;

pub mod reply;
pub mod state;

use self::command::*;
use self::reply::*;
use self::state::*;

static HOSTNAME: &'static str = "fakesmtpd";
static MESSAGE_BODY_TERMINATOR: &'static [u8] = &[b'\r', b'\n', b'.', b'\r', b'\n'];
static INITIAL_MESSAGE_BUFFER_SIZE: usize = 1024 * 4;
static MAX_EMAIL_SIZE: usize = 73_400_320;

lazy_static! {
    static ref MAIL_COMMAND_REGEX: Regex =
        Regex::new("(?i:From):\\s*<(?P<email>[^>]*)>(\\s+(?i:Size)=(?P<size>\\d+))?").unwrap();
    static ref RCPT_COMMAND_REGEX: Regex = Regex::new("(?i:To):\\s*<(?P<email>[^>]+)>").unwrap();
    static ref SIZE: String = format!("SIZE {}", MAX_EMAIL_SIZE);
    static ref EHLO_MESSAGE: Vec<&'static str> = vec![HOSTNAME, SIZE.as_str(), "8BITMIME"];
    static ref GREETING_MESSAGE: String = format!("{} ESMTP ready", HOSTNAME);
}

#[derive(Debug, Default)]
pub struct Protocol {
    pub message: Vec<u8>,
    pub state: State,
    pub last_command: Command,
    pub from: String,
    pub recipients: Vec<String>,

    reject_ratio: f32,
}

impl Protocol {
    pub fn new() -> Self {
        Protocol {
            message: Vec::with_capacity(INITIAL_MESSAGE_BUFFER_SIZE),
            ..Default::default()
        }
    }

    pub fn set_reject_ratio(&mut self, ratio: f32) {
        self.reject_ratio = ratio;
    }

    pub fn is_data(&self) -> bool {
        self.state == State::Data
    }

    pub fn is_done(&self) -> bool {
        self.state == State::Done
    }

    pub fn start(&mut self) -> Reply {
        self.state = State::Establish;
        Reply {
            status: 220,
            lines: vec![GREETING_MESSAGE.as_str()],
        }
    }

    pub fn process_command(&mut self, line: &str) -> Result<Reply, Error> {
        match parse_command(line.trim_right_matches("\r\n")) {
            Ok(cmd) => Ok(self.command(&cmd)),
            Err(err) => Err(err),
        }
    }

    pub fn process_data<R>(&mut self, reader: &mut R) -> Result<Reply, Error>
    where
        R: Read,
    {
        let mut buffer = vec![0u8; INITIAL_MESSAGE_BUFFER_SIZE].into_boxed_slice();

        loop {
            if let Ok(bytes_read) = reader.read(&mut *buffer) {
                if bytes_read > 0 {
                    let m = &mut self.message;
                    if m.len() + bytes_read > MAX_EMAIL_SIZE {
                        return Ok(Reply::message_too_big());
                    }
                    m.extend_from_slice(&(*buffer)[0..bytes_read]);
                    if m.len() >= MESSAGE_BODY_TERMINATOR.len() {
                        let idx = m.len() - MESSAGE_BODY_TERMINATOR.len();
                        if &m[idx..] == MESSAGE_BODY_TERMINATOR {
                            m.truncate(idx + 2);
                            self.state = State::Mail;
                            break;
                        }
                    }
                } else {
                    bail!("client closed connection")
                }
            } else {
                bail!("data read error")
            }
        }

        debug!(
            "received mail to {:?}, size: {}",
            self.recipients,
            self.message.len()
        );

        self.cleanup();

        Ok(Reply::ok("Ok"))
    }

    pub fn command(&mut self, command: &Command) -> Reply {
        match command.verb.as_ref() {
            "QUIT" => {
                self.state = State::Done;
                Reply::bye()
            }
            "NOOP" => Reply::ok("Ok"),
            "RSET" => {
                self.cleanup();
                self.state = State::Mail;
                Reply::ok("Ok")
            }
            "EHLO" if self.state == State::Establish => self.ehlo(),
            "EHLO" if self.state == State::Mail => self.ehlo(),
            "EHLO" if self.state == State::Rcpt => self.ehlo(),
            "HELO" if self.state == State::Establish => self.helo(),
            "HELO" if self.state == State::Mail => self.helo(),
            "HELO" if self.state == State::Rcpt => self.helo(),
            "MAIL" if self.state == State::Mail => self.mail(&command),
            "RCPT" if self.state == State::Rcpt => self.rcpt(&command),
            "DATA" if self.state == State::Rcpt && !self.recipients.is_empty() => self.data(),
            _ => self.invalid_command(),
        }
    }

    fn cleanup(&mut self) {
        self.message.clear();
        self.recipients.clear();
        self.from.clear();
    }

    fn invalid_command(&mut self) -> Reply {
        Reply::unknown_command()
    }

    fn ehlo(&mut self) -> Reply {
        self.state = State::Mail;
        Reply::ok_many(EHLO_MESSAGE.to_vec())
    }

    fn helo(&mut self) -> Reply {
        self.state = State::Mail;
        Reply::ok(HOSTNAME)
    }

    fn mail(&mut self, cmd: &Command) -> Reply {
        self.state = State::Rcpt;
        let cap = MAIL_COMMAND_REGEX.captures(cmd.args.as_str());

        match cap {
            Some(cap) => {
                let addr = cap.name("email").map(|email| email.as_str());
                let size = cap.name("size").map(|size| size.as_str());
                match addr {
                    Some(address) => {
                        self.from = address.to_string();
                        if let Some(size) = size {
                            let size = size.parse::<usize>();
                            match size {
                                Ok(size) if size > MAX_EMAIL_SIZE => Reply::message_too_big(),
                                Err(err) => {
                                    error!("'FROM' command parameter parse error: {}", err);
                                    Reply::unknown_command()
                                }
                                _ => Reply::ok("Ok"),
                            }
                        } else {
                            Reply::ok("Ok")
                        }
                    }
                    None => Reply::invalid_address(),
                }
            }
            None => Reply::invalid_address(),
        }
    }

    fn rcpt(&mut self, cmd: &Command) -> Reply {
        self.state = State::Rcpt;
        let m = RCPT_COMMAND_REGEX
            .captures(cmd.args.as_str())
            .and_then(|cap| cap.name("email").map(|email| email.as_str()));
        match m {
            Some(address) => {
                if self.reject_ratio > 0f32
                    && (self.reject_ratio >= 1f32 || random::<f32>() >= self.reject_ratio)
                {
                    return Reply::unknown_user();
                }
                self.recipients.push(address.to_string());
                Reply::ok("Ok")
            }
            None => Reply::invalid_address(),
        }
    }

    fn data(&mut self) -> Reply {
        self.state = State::Data;
        Reply::data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_command_test1() {
        let mut smtp = Protocol::new();
        let raw = "mail from: <test@example.com> size=432445";
        let cmd = parse_command(raw).unwrap();
        let _ = smtp.mail(&cmd);
        assert_eq!(smtp.from, "test@example.com");
        let reply = smtp.mail(&cmd);
        assert!(reply.status == 250);
    }

    #[test]
    fn mail_command_test2() {
        let mut smtp = Protocol::new();
        let raw = "mail from:<test@example.com>";
        let cmd = parse_command(raw).unwrap();
        let _ = smtp.mail(&cmd);
        assert_eq!(smtp.from, "test@example.com");
        let reply = smtp.mail(&cmd);
        assert!(reply.status == 250);
    }

    #[test]
    fn mail_command_test3() {
        let mut smtp = Protocol::new();
        let raw = "mail from:<>";
        let cmd = parse_command(raw).unwrap();
        let _ = smtp.mail(&cmd);
        assert_eq!(smtp.from, "");
        let reply = smtp.mail(&cmd);
        assert!(reply.status == 250);
    }

    #[test]
    fn mail_command_test4() {
        let mut smtp = Protocol::new();
        let raw = "mail from: <test@example.com> size=432445768556";
        let cmd = parse_command(raw).unwrap();
        let _ = smtp.mail(&cmd);
        assert_eq!(smtp.from, "test@example.com");
        let reply = smtp.mail(&cmd);
        assert!(reply.status == 556);
    }

    #[test]
    fn rcpt_command_test1() {
        let mut smtp = Protocol::new();
        let raw = "rcpt to: <test@example.com>";
        let cmd = parse_command(raw).unwrap();
        let _ = smtp.rcpt(&cmd);
        assert_eq!(smtp.recipients[0], "test@example.com");
        let reply = smtp.rcpt(&cmd);
        assert!(reply.status == 250);
    }

    #[test]
    fn rcpt_command_test2() {
        let mut smtp = Protocol::new();
        let raw = "rcpt to:<test@example.com>";
        let cmd = parse_command(raw).unwrap();
        let _ = smtp.rcpt(&cmd);
        assert_eq!(smtp.recipients[0], "test@example.com");
        let reply = smtp.rcpt(&cmd);
        assert!(reply.status == 250);
    }

    #[test]
    fn rcpt_command_test3() {
        let mut smtp = Protocol::new();
        let raw = "rcpt to:<>";
        let cmd = parse_command(raw).unwrap();
        let reply = smtp.rcpt(&cmd);
        assert!(reply.status > 200);
    }
}
