use anyhow::{anyhow, Error};

#[derive(Debug, Default)]
pub struct Command {
    pub verb: String,
    pub args: String,
    pub origin: String,
}

pub fn parse_command(line: &str) -> Result<Command, Error> {
    let items: Vec<&str> = line.split_whitespace().collect();

    if items.is_empty() {
        Err(anyhow!("invalid command"))
    } else {
        let cmd = Command {
            verb: items[0].to_uppercase(),
            args: items[1..].join(" "),
            origin: line.to_string(),
        };

        Ok(cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_test1() {
        let raw = "mail from: <test@example.com>";
        let cmd = parse_command(raw).unwrap();
        assert_eq!(cmd.verb, "MAIL");
        assert_eq!(cmd.args, "from: <test@example.com>");
        assert_eq!(cmd.origin, raw);
    }

    #[test]
    fn parse_command_test2() {
        let raw = "data";
        let cmd = parse_command(raw).unwrap();
        assert_eq!(cmd.verb, "DATA");
        assert_eq!(cmd.args, "");
        assert_eq!(cmd.origin, raw);
    }
}
