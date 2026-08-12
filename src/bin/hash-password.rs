use std::io::{self, Read};

use argon2::{
    password_hash::{SaltString, PasswordHasher},
    Argon2,
};
use rand::rngs::OsRng;

fn main() -> anyhow::Result<()> {
    let mut password = String::new();
    io::stdin().read_to_string(&mut password)?;
    let password = password.trim_end_matches(['\r', '\n']);
    if password.len() < 12 {
        anyhow::bail!("password must contain at least 12 characters");
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    println!("{hash}");
    Ok(())
}
