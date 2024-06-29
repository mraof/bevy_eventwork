use crate::error::NetworkError;

/// A trait that provides serialization functions for the network
pub trait NetworkSerializer: Send + Sync + 'static {
    /// Serializes the given type into bytes
    fn serialize<T: ?Sized>(value: &T) -> Result<Vec<u8>, NetworkError>
    where
        T: serde::Serialize;

    /// Deserializes the given bytes into the type
    fn deserialize<'a, T>(bytes: &'a [u8]) -> Result<T, NetworkError>
    where
        T: serde::de::Deserialize<'a>;
}

/// An implementation of [`NetworkSerializer`] using Bincode
#[derive(Default)]
pub struct BincodeSerializer;

impl NetworkSerializer for BincodeSerializer {
    fn serialize<T: ?Sized>(value: &T) -> Result<Vec<u8>, NetworkError>
    where
        T: serde::Serialize,
    {
        bincode::serialize(value).map_err(|_| NetworkError::Serialization)
    }

    fn deserialize<'a, T>(bytes: &'a [u8]) -> Result<T, NetworkError>
    where
        T: serde::de::Deserialize<'a>,
    {
        bincode::deserialize(bytes).map_err(|_| NetworkError::Serialization)
    }
}
