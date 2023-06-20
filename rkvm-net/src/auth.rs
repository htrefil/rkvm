use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use rand::{Error, Rng};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::task;

type ChallengeHmac = Hmac<Sha256>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AuthChallenge([u8; 32]);

impl AuthChallenge {
    pub async fn generate() -> Result<Self, Error> {
        task::spawn_blocking(|| {
            let mut data = [0; 32];
            OsRng.try_fill(&mut data)?;

            Ok(Self(data))
        })
        .await
        .unwrap()
    }

    pub fn respond(&self, password: &str) -> AuthResponse {
        let mut mac = ChallengeHmac::new_from_slice(password.as_bytes()).unwrap();
        mac.update(&self.0);

        let result = mac.finalize();
        let result = result.into_bytes();
        let result = result[..].try_into().unwrap();

        AuthResponse(result)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AuthResponse([u8; 32]);

impl AuthResponse {
    pub fn verify(&self, challenge: &AuthChallenge, password: &str) -> bool {
        let mut mac = ChallengeHmac::new_from_slice(password.as_bytes()).unwrap();
        mac.update(&challenge.0);

        mac.verify_slice(&self.0).is_ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum AuthStatus {
    Passed,
    Failed,
}
