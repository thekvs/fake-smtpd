use std::fmt;

// #[derive(Debug)]
// enum Lines<'a> {
//     ONE(&'a str),
//     MANY(Vec<&'a str>),
// }

#[derive(Debug, Default)]
pub struct Reply<'a> {
    pub status: u16,
    pub lines: Vec<&'a str>,
}

impl<'a> fmt::Display for Reply<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let len = self.lines.len();
        if len > 0 {
            for idx in 0..len - 1 {
                fmt.write_fmt(format_args!("{}-{}\r\n", self.status, self.lines[idx]))?;
            }
            fmt.write_fmt(format_args!("{} {}\r\n", self.status, self.lines[len - 1]))?;
        }
        Ok(())
    }
}

impl<'a> Reply<'a> {
    pub fn ok(message: &'a str) -> Self {
        Reply {
            status: 250,
            lines: vec![message],
        }
    }

    pub fn ok_many(messages: Vec<&'a str>) -> Self {
        Reply {
            status: 250,
            lines: messages,
        }
    }

    pub fn bye() -> Self {
        Reply {
            status: 221,
            lines: vec!["Bye"],
        }
    }

    pub fn data() -> Self {
        Reply {
            status: 354,
            lines: vec!["End data with <CR><LF>.<CR><LF>"],
        }
    }

    pub fn unknown_command() -> Self {
        Reply {
            status: 500,
            lines: vec!["Invalid or out of order command"],
        }
    }

    pub fn invalid_address() -> Self {
        Reply {
            status: 502,
            lines: vec!["Malformed email address"],
        }
    }

    pub fn message_too_big() -> Self {
        Reply {
            status: 556,
            lines: vec!["Message size exceeds maximum allowed"],
        }
    }

    pub fn unknown_user() -> Self {
        Reply {
            status: 550,
            lines: vec!["User unknown"],
        }
    }
}
