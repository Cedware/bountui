use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Read;
use std::path::Path;

#[derive(Serialize, Deserialize, Default)]
struct UserInputs {
    local_ports: HashMap<String, u16>,
}


#[cfg_attr(test, mockall::automock)]
pub trait RememberUserInput {
    fn store_local_port(&mut self, target: String, port: u16) -> anyhow::Result<()>;
    fn get_local_port(&self, target_id: &String) -> anyhow::Result<Option<u16>>;
}

fn read_user_inputs<P: AsRef<Path>>(path: P) -> anyhow::Result<UserInputs> {
    if !path.as_ref().exists() {
        return Ok(UserInputs::default());
    }
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .context("Failed to open file")?;
    let mut file_content: String = String::new();
    file.read_to_string(&mut file_content)
        .context("Failed to read from file")?;
    if file_content.is_empty() {
        Ok(UserInputs::default())
    } else {
        Ok(serde_json::from_str(&file_content).context("Failed to parse json")?)
    }
}

fn write_user_inputs<P: AsRef<Path>>(path: P, user_inputs: &UserInputs) -> anyhow::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        create_dir_all(parent).context("Failed to create parent directories")?;
    }
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .context("Failed to open file")?;
    serde_json::to_writer_pretty(file, user_inputs).context("Failed to write json")?;
    Ok(())
}

#[derive(Copy, Clone)]
pub struct UserInputsPath<P>(pub P);

impl<P: AsRef<Path>> From<P> for UserInputsPath<P> {
    fn from(value: P) -> Self {
        UserInputsPath(value)
    }
}

impl<P> RememberUserInput for UserInputsPath<P>
where
    P: AsRef<Path>,
{
    fn store_local_port(&mut self, target: String, port: u16) -> anyhow::Result<()> {
        let mut user_inputs =
            read_user_inputs(self.0.as_ref()).context("Failed to read user inputs")?;
        user_inputs.local_ports.insert(target, port);
        write_user_inputs(self.0.as_ref(), &user_inputs)
    }

    fn get_local_port(&self, target_id: &String) -> anyhow::Result<Option<u16>> {
        Ok(read_user_inputs(self.0.as_ref())
            .context("Failed to read user inputs")?
            .local_ports
            .get(target_id)
            .copied())
    }
}

impl<P> RememberUserInput for Option<P>
where
    P: RememberUserInput,
{
    fn store_local_port(&mut self, target: String, port: u16) -> anyhow::Result<()> {
        if let Some(inner_self) = self {
            inner_self.store_local_port(target, port)
        } else {
            Ok(())
        }
    }

    fn get_local_port(&self, target_id: &String) -> anyhow::Result<Option<u16>> {
        if let Some(inner_self) = self {
            inner_self.get_local_port(target_id)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::bountui::{RememberUserInput, UserInputsPath};
    use std::io::Write;
    use std::path::Path;
    use tempfile::NamedTempFile;

    const JSON: &str = "{\"local_ports\": {\"target_id\": 8080}}";

    fn create_user_input_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(JSON.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_get_local_port_file_does_not_exist() {
        let path = UserInputsPath(Path::new("/does/not/exist"));
        let port = path.get_local_port(&"target_id".to_string()).unwrap();
        assert!(port.is_none());
    }

    #[test]
    fn test_get_local_port_for_target_that_is_not_stored() {
        let file = create_user_input_file();
        let path = UserInputsPath(file.path());
        let port = path.get_local_port(&"unknown_target_id".to_string()).unwrap();
        assert!(port.is_none());
    }

    #[test]
    fn test_get_local_port_for_target_that_is_stored() {
        let file = create_user_input_file();
        let path = UserInputsPath(file.path());
        let port = path.get_local_port(&"target_id".to_string()).unwrap();
        assert_eq!(Some(8080), port);
    }

    #[test]
    fn store_local_port_and_get_local_port() {
        let file = NamedTempFile::new().unwrap();
        let mut path = UserInputsPath(file.path());
        path.store_local_port("target_id_1".to_string(), 8080).unwrap();
        path.store_local_port("target_id_2".to_string(), 8081).unwrap();
        let target_id_1_port = path.get_local_port(&"target_id_1".to_string()).unwrap();
        let target_id_2_port = path.get_local_port(&"target_id_2".to_string()).unwrap();
        assert_eq!(Some(8080), target_id_1_port);
        assert_eq!(Some(8081), target_id_2_port);
    }
}