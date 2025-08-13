use serde::{Deserialize, Serialize};
use std::io::{self, BufReader, Read};
use std::mem;
use std::path::Path;
use std::{assert, fs::File, slice};

const PROFILE_USERNAME_SIZE: usize = 32;
const MAX_USERS: usize = 8;

type ProfileUsername = [u8; PROFILE_USERNAME_SIZE];
type Uuid = [u64; 2];

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct UserRaw {
    uuid: Uuid,
    uuid2: Uuid,
    timestamp: u64,
    username: ProfileUsername,
    _extra_data: [u8; 0x80],
}

#[repr(C)]
#[derive(Debug, Clone)]
struct ProfileDataRaw {
    _padding: [u8; 0x10],
    users: [UserRaw; MAX_USERS],
}

fn read_struct<T, R: Read>(mut read: R) -> io::Result<T> {
    let num_bytes = std::mem::size_of::<T>();
    unsafe {
        let mut s = mem::zeroed();
        let buffer = slice::from_raw_parts_mut(&mut s as *mut T as *mut u8, num_bytes);
        match read.read_exact(buffer) {
            Ok(()) => Ok(s),
            Err(e) => {
                std::mem::forget(s);
                Err(e)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserProfile {
    pub name: String,
    uuid: [String; 2],
}

impl UserProfile {
    pub fn get_uuid_emu_storage_string(&self) -> String {
        let uuid1: u64 = self.uuid[0].parse().expect("Unable to parse uuid into u64");
        let uuid2: u64 = self.uuid[1].parse().expect("Unable to parse uuid into u64");
        format!("{:016X}{:016X}", uuid2, uuid1).to_string()
    }
    pub fn get_uuid_arc_storage_strings(&self) -> (String, String) {
        (self.uuid[0].to_string(), self.uuid[1].to_string())
    }
}

pub fn parse_user_profiles_save_file(nand_dir: &Path) -> io::Result<Vec<UserProfile>> {
    let profile_raw_data_size = mem::size_of::<ProfileDataRaw>();
    assert!(
        profile_raw_data_size == 0x650,
        "ProfileDataRaw has wrong struct size: {}",
        profile_raw_data_size,
    );
    let user_profile_save = nand_dir
        .join("system")
        .join("save")
        .join("8000000000000010")
        .join("su")
        .join("avators")
        .join("profiles.dat");
    log::info!(
        "Trying to use use profile data file: {:?}",
        user_profile_save
    );

    let save = File::open(user_profile_save)?;

    let reader = BufReader::new(save);
    let data = read_struct::<ProfileDataRaw, _>(reader)?;

    let mut user_profiles = vec![];

    for user in data.users {
        if user.uuid[0] == 0 && user.uuid[1] == 0 {
            continue;
        }

        let uuid_str = [
            format!("{}", user.uuid[0]).to_string(),
            format!("{}", user.uuid[1]).to_string(),
        ];
        user_profiles.push(UserProfile {
            name: std::str::from_utf8(&user.username)
                .expect("Unable to convert username to utf8")
                .trim_end_matches('\0')
                .to_string(),
            uuid: uuid_str,
        });
    }

    Ok(user_profiles)
}
