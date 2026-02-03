use super::SysCaller;
use crate::ArgsMap;
use syscalls::Errno;

#[derive(Debug)]
pub struct CapsetCall {}

impl CapsetCall {
    pub fn new(_args: &ArgsMap) -> Self {
        Self {}
    }
}

impl SysCaller for CapsetCall {
    fn call(&self) -> Result<usize, Errno> {
        match caps::set(
            None,
            caps::CapSet::Effective,
            &[caps::Capability::CAP_SYS_ADMIN].into(),
        ) {
            Ok(_) => Ok(0),
            Err(_) => Err(Errno::new(
                std::io::Error::last_os_error().raw_os_error().unwrap(),
            )),
        }
    }
}
