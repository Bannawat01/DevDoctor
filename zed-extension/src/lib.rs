use zed_extension_api::{self as zed, Command, ContextServerId, Project, Result};

struct DevDoctorExtension;

impl zed::Extension for DevDoctorExtension {
    fn new() -> Self {
        Self
    }

    fn context_server_command(
        &mut self,
        _context_server_id: &ContextServerId,
        _project: &Project,
    ) -> Result<Command> {
        Ok(Command {
            command: "devdoctor-mcp".into(),
            args: Vec::new(),
            env: Vec::new(),
        })
    }
}

zed::register_extension!(DevDoctorExtension);
