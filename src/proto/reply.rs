use std::fmt;

// #[derive(Debug)]
// enum Lines<'a> {
//     ONE(&'a str),
//     MANY(Vec<&'a str>),
// }

static OK_STATUS_CODE: u16 = 250;
static BYE_STATUS_CODE: u16 = 221;
static DATA_STATUS_CODE: u16 = 354;
static UNKNOWN_COMMAND_STATUS_CODE: u16 = 500;
static INVALID_ADDRESS_STATUS_CODE: u16 = 502;
static MESSAGE_TOO_BIG_STATUS_CODE: u16 = 556;
static UNKNOWN_USER_STATUS_CODE: u16 = 550;
static TOO_MANY_RECIPIENTS_STATUS_CODE: u16 = 452;

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
            status: OK_STATUS_CODE,
            lines: vec![message],
        }
    }

    pub fn ok_many(messages: Vec<&'a str>) -> Self {
        Reply {
            status: OK_STATUS_CODE,
            lines: messages,
        }
    }

    pub fn bye() -> Self {
        Reply {
            status: BYE_STATUS_CODE,
            lines: vec!["Bye"],
        }
    }

    pub fn data() -> Self {
        Reply {
            status: DATA_STATUS_CODE,
            lines: vec!["End data with <CR><LF>.<CR><LF>"],
        }
    }

    pub fn unknown_command() -> Self {
        Reply {
            status: UNKNOWN_COMMAND_STATUS_CODE,
            lines: vec!["Invalid or out of order command"],
        }
    }

    pub fn invalid_address() -> Self {
        Reply {
            status: INVALID_ADDRESS_STATUS_CODE,
            lines: vec!["Malformed email address"],
        }
    }

    pub fn message_too_big() -> Self {
        Reply {
            status: MESSAGE_TOO_BIG_STATUS_CODE,
            lines: vec!["Message size exceeds maximum allowed"],
        }
    }

    pub fn unknown_user() -> Self {
        Reply {
            status: UNKNOWN_USER_STATUS_CODE,
            lines: vec!["User unknown"],
        }
    }

    pub fn too_many_recipients() -> Self {
        Reply {
            status: TOO_MANY_RECIPIENTS_STATUS_CODE,
            lines: vec!["Too many recipients"],
        }
    }
}
