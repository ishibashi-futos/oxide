#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: String,
    pub args: Vec<String>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommandError {
    MissingSlash,
    MissingName,
}

pub fn parse_slash_command(input: &str) -> Result<SlashCommand, SlashCommandError> {
    let trimmed = input.trim();
    let Some(stripped) = trimmed.strip_prefix('/') else {
        return Err(SlashCommandError::MissingSlash);
    };
    let mut parts = stripped.split_whitespace();
    let Some(name) = parts.next() else {
        return Err(SlashCommandError::MissingName);
    };
    if name.is_empty() {
        return Err(SlashCommandError::MissingName);
    }
    let args = parts.map(|part| part.to_string()).collect();
    Ok(SlashCommand {
        name: name.to_string(),
        args,
        raw: trimmed.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_slash_command_parses_preview() {
        let command = parse_slash_command("/preview").unwrap();

        assert_eq!(command.name, "preview");
        assert!(command.args.is_empty());
        assert_eq!(command.raw, "/preview");
    }

    #[test]
    fn parse_slash_command_parses_preview_hide() {
        let command = parse_slash_command("/preview hide").unwrap();

        assert_eq!(command.name, "preview");
        assert_eq!(command.args, vec!["hide".to_string()]);
        assert_eq!(command.raw, "/preview hide");
    }

    #[test]
    fn parse_slash_command_parses_preview_show() {
        let command = parse_slash_command("/preview show").unwrap();

        assert_eq!(command.name, "preview");
        assert_eq!(command.args, vec!["show".to_string()]);
        assert_eq!(command.raw, "/preview show");
    }
}
