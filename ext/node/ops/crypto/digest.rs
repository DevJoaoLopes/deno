// Copyright 2018-2025 the Deno authors. MIT license.
use std::cell::RefCell;
use std::rc::Rc;

use deno_core::GarbageCollected;
use deno_core::op2;
use digest::Digest;
use digest::DynDigest;
use digest::ExtendableOutput;
use digest::Update;

mod ring_sha2;

pub struct Hasher {
  pub hash: Rc<RefCell<Option<Hash>>>,
}

impl GarbageCollected for Hasher {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"Hasher"
  }
}

// Make prototype available for JavaScript
#[op2]
impl Hasher {
  #[constructor]
  #[cppgc]
  fn create(_: bool) -> Hasher {
    unreachable!()
  }
}

impl Hasher {
  pub fn new(
    algorithm: &str,
    output_length: Option<usize>,
  ) -> Result<Self, HashError> {
    let hash = Hash::new(algorithm, output_length)?;

    Ok(Self {
      hash: Rc::new(RefCell::new(Some(hash))),
    })
  }

  pub fn update(&self, data: &[u8]) -> bool {
    if let Some(hash) = self.hash.borrow_mut().as_mut() {
      hash.update(data);
      true
    } else {
      false
    }
  }

  pub fn digest(&self) -> Option<Box<[u8]>> {
    let hash = self.hash.borrow_mut().take()?;
    Some(hash.digest_and_drop())
  }

  pub fn clone_inner(
    &self,
    output_length: Option<usize>,
  ) -> Result<Option<Self>, HashError> {
    let hash = self.hash.borrow();
    let Some(hash) = hash.as_ref() else {
      return Ok(None);
    };
    let hash = hash.clone_hash(output_length)?;
    Ok(Some(Self {
      hash: Rc::new(RefCell::new(Some(hash))),
    }))
  }
}

macro_rules! match_fixed_digest {
  ($algorithm_name:expr, fn <$type:ident>() $body:block, _ => $other:block) => {
    match $algorithm_name {
      "blake2b512" => {
        type $type = ::blake2::Blake2b512;
        $body
      }
      "blake2s256" => {
        type $type = ::blake2::Blake2s256;
        $body
      }
      #[allow(dead_code)]
      _ => crate::ops::crypto::digest::match_fixed_digest_with_eager_block_buffer!($algorithm_name, fn <$type>() $body, _ => $other)
    }
  };
}
pub(crate) use match_fixed_digest;

macro_rules! match_fixed_digest_with_eager_block_buffer {
  ($algorithm_name:expr, fn <$type:ident>() $body:block, _ => $other:block) => {
    match $algorithm_name {
      "rsa-sm3" | "sm3" | "sm3withrsaencryption" => {
        type $type = ::sm3::Sm3;
        $body
      }
      "rsa-md4" | "md4" | "md4withrsaencryption" => {
        type $type = ::md4::Md4;
        $body
      }
      "md5-sha1" => {
        type $type = crate::ops::crypto::md5_sha1::Md5Sha1;
        $body
      }
      _ => crate::ops::crypto::digest::match_fixed_digest_with_oid!($algorithm_name, fn <$type>() $body, _ => $other)
    }
  };
}
pub(crate) use match_fixed_digest_with_eager_block_buffer;

macro_rules! match_fixed_digest_with_oid {
  ($algorithm_name:expr, fn $(<$type:ident>)?($($hash_algorithm:ident: Option<RsaPssHashAlgorithm>)?) $body:block, _ => $other:block) => {
    match $algorithm_name {
      "rsa-md5" | "md5" | "md5withrsaencryption" | "ssl3-md5" => {
        $(let $hash_algorithm = None;)?
        $(type $type = ::md5::Md5;)?
        $body
      }
      "rsa-ripemd160" | "ripemd" | "ripemd160" | "ripemd160withrsa"
      | "rmd160" => {
        $(let $hash_algorithm = None;)?
        $(type $type = ::ripemd::Ripemd160;)?
        $body
      }
      "rsa-sha1"
      | "rsa-sha1-2"
      | "sha1"
      | "sha1-2"
      | "sha1withrsaencryption"
      | "ssl3-sha1" => {
        $(let $hash_algorithm = Some(RsaPssHashAlgorithm::Sha1);)?
        $(type $type = ::sha1::Sha1;)?
        $body
      }
      "rsa-sha224" | "sha224" | "sha224withrsaencryption" => {
        $(let $hash_algorithm = Some(RsaPssHashAlgorithm::Sha224);)?
        $(type $type = ::sha2::Sha224;)?
        $body
      }
      "rsa-sha256" | "sha256" | "sha256withrsaencryption" => {
        $(let $hash_algorithm = Some(RsaPssHashAlgorithm::Sha256);)?
        $(type $type = ::sha2::Sha256;)?
        $body
      }
      "rsa-sha384" | "sha384" | "sha384withrsaencryption" => {
        $(let $hash_algorithm = Some(RsaPssHashAlgorithm::Sha384);)?
        $(type $type = ::sha2::Sha384;)?
        $body
      }
      "rsa-sha512" | "sha512" | "sha512withrsaencryption" => {
        $(let $hash_algorithm = Some(RsaPssHashAlgorithm::Sha512);)?
        $(type $type = ::sha2::Sha512;)?
        $body
      }
      "rsa-sha512/224" | "sha512-224" | "sha512-224withrsaencryption" => {
        $(let $hash_algorithm = Some(RsaPssHashAlgorithm::Sha512_224);)?
        $(type $type = ::sha2::Sha512_224;)?
        $body
      }
      "rsa-sha512/256" | "sha512-256" | "sha512-256withrsaencryption" => {
        $(let $hash_algorithm = Some(RsaPssHashAlgorithm::Sha512_256);)?
        $(type $type = ::sha2::Sha512_256;)?
        $body
      }
      "rsa-sha3-224" | "id-rsassa-pkcs1-v1_5-with-sha3-224" | "sha3-224" => {
        $(let $hash_algorithm = None;)?
        $(type $type = ::sha3::Sha3_224;)?
        $body
      }
      "rsa-sha3-256" | "id-rsassa-pkcs1-v1_5-with-sha3-256" | "sha3-256" => {
        $(let $hash_algorithm = None;)?
        $(type $type = ::sha3::Sha3_256;)?
        $body
      }
      "rsa-sha3-384" | "id-rsassa-pkcs1-v1_5-with-sha3-384" | "sha3-384" => {
        $(let $hash_algorithm = None;)?
        $(type $type = ::sha3::Sha3_384;)?
        $body
      }
      "rsa-sha3-512" | "id-rsassa-pkcs1-v1_5-with-sha3-512" | "sha3-512" => {
        $(let $hash_algorithm = None;)?
        $(type $type = ::sha3::Sha3_512;)?
        $body
      }
      _ => $other,
    }
  };
}

pub(crate) use match_fixed_digest_with_oid;

pub enum Hash {
  FixedSize(Box<dyn DynDigest>),

  Shake128(Box<sha3::Shake128>, /* output_length: */ Option<usize>),
  Shake256(Box<sha3::Shake256>, /* output_length: */ Option<usize>),
}

use Hash::*;

#[derive(Debug, thiserror::Error, deno_error::JsError)]
#[class(generic)]
pub enum HashError {
  #[error("Output length mismatch for non-extendable algorithm")]
  OutputLengthMismatch,
  #[error("Digest method not supported: {0}")]
  DigestMethodUnsupported(String),
}

impl Hash {
  pub fn new(
    algorithm_name: &str,
    output_length: Option<usize>,
  ) -> Result<Self, HashError> {
    match algorithm_name {
      "shake128" | "shake-128" => {
        return Ok(Shake128(Default::default(), output_length));
      }
      "shake256" | "shake-256" => {
        return Ok(Shake256(Default::default(), output_length));
      }
      "sha256" => {
        let digest = ring_sha2::RingSha256::new();
        if let Some(length) = output_length {
          if length != digest.output_size() {
            return Err(HashError::OutputLengthMismatch);
          }
        }
        return Ok(Hash::FixedSize(Box::new(digest)));
      }
      "sha512" => {
        let digest = ring_sha2::RingSha512::new();
        if let Some(length) = output_length {
          if length != digest.output_size() {
            return Err(HashError::OutputLengthMismatch);
          }
        }
        return Ok(Hash::FixedSize(Box::new(digest)));
      }
      _ => {}
    }

    let algorithm = match_fixed_digest!(
      algorithm_name,
      fn <D>() {
        let digest: D = Digest::new();
        if let Some(length) = output_length {
          if length != digest.output_size() {
            return Err(HashError::OutputLengthMismatch);
          }
        }
        FixedSize(Box::new(digest))
      },
      _ => {
        return Err(HashError::DigestMethodUnsupported(algorithm_name.to_string()))
      }
    );

    Ok(algorithm)
  }

  pub fn update(&mut self, data: &[u8]) {
    match self {
      FixedSize(context) => DynDigest::update(&mut **context, data),
      Shake128(context, _) => Update::update(&mut **context, data),
      Shake256(context, _) => Update::update(&mut **context, data),
    };
  }

  pub fn digest_and_drop(self) -> Box<[u8]> {
    match self {
      FixedSize(context) => context.finalize(),

      // The default output lengths align with Node.js
      Shake128(context, output_length) => {
        context.finalize_boxed(output_length.unwrap_or(16))
      }
      Shake256(context, output_length) => {
        context.finalize_boxed(output_length.unwrap_or(32))
      }
    }
  }

  pub fn clone_hash(
    &self,
    output_length: Option<usize>,
  ) -> Result<Self, HashError> {
    let hash = match self {
      FixedSize(context) => {
        if let Some(length) = output_length {
          if length != context.output_size() {
            return Err(HashError::OutputLengthMismatch);
          }
        }
        FixedSize(context.box_clone())
      }

      Shake128(context, _) => Shake128(context.clone(), output_length),
      Shake256(context, _) => Shake256(context.clone(), output_length),
    };
    Ok(hash)
  }

  pub fn get_hashes() -> Vec<&'static str> {
    vec![
      "RSA-MD4",
      "RSA-MD5",
      "RSA-RIPEMD160",
      "RSA-SHA1",
      "RSA-SHA1-2",
      "RSA-SHA224",
      "RSA-SHA256",
      "RSA-SHA3-224",
      "RSA-SHA3-256",
      "RSA-SHA3-384",
      "RSA-SHA3-512",
      "RSA-SHA384",
      "RSA-SHA512",
      "RSA-SHA512/224",
      "RSA-SHA512/256",
      "RSA-SM3",
      "blake2b512",
      "blake2s256",
      "id-rsassa-pkcs1-v1_5-with-sha3-224",
      "id-rsassa-pkcs1-v1_5-with-sha3-256",
      "id-rsassa-pkcs1-v1_5-with-sha3-384",
      "id-rsassa-pkcs1-v1_5-with-sha3-512",
      "md4",
      "md4WithRSAEncryption",
      "md5",
      "md5-sha1",
      "md5WithRSAEncryption",
      "ripemd",
      "ripemd160",
      "ripemd160WithRSA",
      "rmd160",
      "sha1",
      "sha1WithRSAEncryption",
      "sha224",
      "sha224WithRSAEncryption",
      "sha256",
      "sha256WithRSAEncryption",
      "sha3-224",
      "sha3-256",
      "sha3-384",
      "sha3-512",
      "sha384",
      "sha384WithRSAEncryption",
      "sha512",
      "sha512-224",
      "sha512-224WithRSAEncryption",
      "sha512-256",
      "sha512-256WithRSAEncryption",
      "sha512WithRSAEncryption",
      "shake128",
      "shake256",
      "sm3",
      "sm3WithRSAEncryption",
      "ssl3-md5",
      "ssl3-sha1",
    ]
  }

  pub fn get_size(algorithm_name: &str) -> Option<u8> {
    match algorithm_name {
      "RSA-MD4" => Some(16),
      "RSA-MD5" => Some(16),
      "RSA-RIPEMD160" => Some(20),
      "RSA-SHA1" => Some(20),
      "RSA-SHA1-2" => Some(20),
      "RSA-SHA224" => Some(28),
      "RSA-SHA256" => Some(32),
      "RSA-SHA3-224" => Some(28),
      "RSA-SHA3-256" => Some(32),
      "RSA-SHA3-384" => Some(48),
      "RSA-SHA3-512" => Some(64),
      "RSA-SHA384" => Some(48),
      "RSA-SHA512" => Some(64),
      "RSA-SHA512/224" => Some(28),
      "RSA-SHA512/256" => Some(32),
      "RSA-SM3" => Some(32),
      "blake2b512" => Some(64),
      "blake2s256" => Some(32),
      "id-rsassa-pkcs1-v1_5-with-sha3-224" => Some(28),
      "id-rsassa-pkcs1-v1_5-with-sha3-256" => Some(32),
      "id-rsassa-pkcs1-v1_5-with-sha3-384" => Some(48),
      "id-rsassa-pkcs1-v1_5-with-sha3-512" => Some(64),
      "md4" => Some(16),
      "md4WithRSAEncryption" => Some(16),
      "md5" => Some(16),
      "md5-sha1" => Some(20),
      "md5WithRSAEncryption" => Some(16),
      "ripemd" => Some(20),
      "ripemd160" => Some(20),
      "ripemd160WithRSA" => Some(20),
      "rmd160" => Some(20),
      "sha1" => Some(20),
      "sha1WithRSAEncryption" => Some(20),
      "sha224" => Some(28),
      "sha224WithRSAEncryption" => Some(28),
      "sha256" => Some(32),
      "sha256WithRSAEncryption" => Some(32),
      "sha3-224" => Some(28),
      "sha3-256" => Some(32),
      "sha3-384" => Some(48),
      "sha3-512" => Some(64),
      "sha384" => Some(48),
      "sha384WithRSAEncryption" => Some(48),
      "sha512" => Some(64),
      "sha512-224" => Some(28),
      "sha512-224WithRSAEncryption" => Some(28),
      "sha512-256" => Some(32),
      "sha512-256WithRSAEncryption" => Some(32),
      "sha512WithRSAEncryption" => Some(64),
      "shake128" => None, // Variable length
      "shake256" => None, // Variable length
      "sm3" => Some(32),
      "sm3WithRSAEncryption" => Some(32),
      "ssl3-md5" => Some(16),
      "ssl3-sha1" => Some(20),
      _ => None,
    }
  }
}
