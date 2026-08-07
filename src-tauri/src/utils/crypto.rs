use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use rand::RngCore;

use crate::utils::error::AppError;
use crate::utils::paths;

const KEY_FILE: &str = "secret.key";
const NONCE_LEN: usize = 12;

/// AES-256-GCM 加密 API Key
pub fn encrypt_key(plaintext: &str) -> Result<String, AppError> {
    let key = load_or_create_key()?;
    let cipher = Aes256Gcm::new((&key).into());
    let nonce_bytes = random_bytes::<NONCE_LEN>();
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| AppError::Internal("加密失败".into()))?;
    // 格式: nonce(12) || ciphertext，base64 编码
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(base64::engine::general_purpose::STANDARD.encode(&combined))
}

/// 解密 API Key；失败返回 Err（调用方可按明文兼容处理）
pub fn decrypt_key(encoded: &str) -> Result<String, AppError> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| AppError::Internal("密钥格式错误".into()))?;
    if data.len() < NONCE_LEN + 1 {
        return Err(AppError::Internal("密钥数据过短".into()));
    }
    let (nonce_bytes, ciphertext) = data.split_at(NONCE_LEN);
    let key = load_or_create_key()?;
    let cipher = Aes256Gcm::new((&key).into());
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| AppError::Internal("密钥解密失败（可能为非加密存储的旧数据）".into()))?;
    String::from_utf8(plaintext).map_err(|_| AppError::Internal("密钥内容非法".into()))
}

/// 密钥持久化在 app_data_dir/secret.key；不存在则生成 32 字节随机密钥
fn load_or_create_key() -> Result<[u8; 32], AppError> {
    let path = paths::app_data_dir().join(KEY_FILE);
    if let Ok(data) = std::fs::read(&path) {
        if data.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&data);
            return Ok(key);
        }
    }
    let key = random_bytes::<32>();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &key)?;
    Ok(key)
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}
