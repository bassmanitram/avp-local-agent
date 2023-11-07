//! Implements the `PolicySetSource` for Amazon Verified Permissions.
use async_trait::async_trait;

pub mod cache;
pub mod policy;
mod retry;
pub mod schema;
pub mod template;

/*
    Retry AVP API calls for a max of 5 seconds
    There is some randomness in the exponential backoff algorithm but this will likely result in
    a maximum of 4-6 retries in the worst case
*/
pub static API_RETRY_TIMEOUT_IN_SECONDS: u64 = 5;

/// Type values for cache changes
#[derive(Debug, Eq, PartialEq)]
pub enum CacheChange {
    /// `Created` indicates a new cache item was created
    Created,
    /// `Updated` indicates an existing cache item was updated
    Updated,
    /// `Deleted` indicates an existing cache item was deleted
    Deleted,
}

/// `Load` trait for AVP callers to retrieve lists of policy store data
#[async_trait]
pub trait Load {
    /// `Input` id of policy store data
    type Input;
    /// `Output` collection of AVP "Item" types retrieved with loader such as `PolicyItem`
    type Output;
    /// `Exception` AVP error types mapped to a loader exception
    type Exception;
    /// Loader method to retrieve a list of policy store items from AVP
    async fn load(&self, input: Self::Input) -> Result<Self::Output, Self::Exception>;
}

/// `Read` trait for callers to retrieve policy store data from AVP.
#[async_trait]
pub trait Read {
    /// `Input` id of policy store data
    type Input;
    /// `Output` data value of "GetOutput" types retrieved with reader such as `GetPolicyOutput`
    type Output;
    /// `Exception` AVP error types mapped to a reader exception
    type Exception;

    /// Reader method to retrieve a policy store output from AVP
    async fn read(&self, input: Self::Input) -> Result<Self::Output, Self::Exception>;
}

/// Cache trait that stores various items from the AVP policy store
/// This trait is limited to a non-thread safe cache as the `get` function returns a reference
/// which cannot protect internal state using a Mutex/RwLock
#[async_trait]
pub trait Cache {
    /// `Key` id of policy store data
    type Key;
    /// `Value` data of caches with types from response of AVP read calls such as from the policy reader
    type Value;
    /// `LoadedItems` HashMap of id, value pairings of `Key` and cache item types from
    /// AVP load calls such as `ListPolicies` returning `PolicyItem`
    type LoadedItems;
    /// `PendingUpdates` HashMap of id, value pairings of `Key` and `CacheChange` as a reference for which
    /// cache values need updates
    type PendingUpdates;

    /// Constructor for cache
    fn new() -> Self;

    /// Getter method for cache, returns reference to value in cache which is not thread safe
    fn get(&self, key: &Self::Key) -> Option<&Self::Value>;

    /// Insert method for cache which takes a `Key` and `Value` pair
    fn put(&mut self, key: Self::Key, value: Self::Value) -> Option<Self::Value>;

    /// Remove method for cache which returns the deleted value
    fn remove(&mut self, key: &Self::Key) -> Option<Self::Value>;

    /// The function responsible for cross checking the values of current cache and returning
    /// a HashMap of values that require an update
    fn get_pending_updates(&self, ids_map: &Self::LoadedItems) -> Self::PendingUpdates;
}

#[cfg(test)]
mod test {
    use aws_credential_types::Credentials;
    use aws_sdk_verifiedpermissions::{Client, Config};
    use aws_smithy_client::test_connection::TestConnection;
    use aws_smithy_http::body::SdkBody;
    use aws_types::region::Region;
    use http::{Request, Response, StatusCode};
    use serde::Serialize;

    /// A pair of a request and responses for the mock AVP client.
    pub type RequestResponsePair = (Request<SdkBody>, Response<SdkBody>);

    /// Builds a mock AVP client with the provided events
    pub fn build_client(events: Vec<RequestResponsePair>) -> Client {
        let conf = Config::builder()
            .credentials_provider(Credentials::new("a", "b", Some("c".to_string()), None, "d"))
            .region(Region::new("us-east-1"))
            .http_connector(TestConnection::new(events))
            .build();

        Client::from_conf(conf)
    }

    /// Builds an event from the provided serializable request and response and status code to be
    /// used with a mock AVP client.
    ///
    /// # Panics
    ///
    /// Will panic if failing to convert `request` to `SdkBody`
    pub fn build_event<S, T>(
        request: &S,
        response: &T,
        status_code: StatusCode,
    ) -> RequestResponsePair
    where
        S: ?Sized + Serialize,
        T: ?Sized + Serialize,
    {
        let request = Request::new(SdkBody::from(serde_json::to_string(&request).unwrap()));

        let response = Response::builder()
            .status(status_code)
            .body(SdkBody::from(serde_json::to_string(&response).unwrap()))
            .unwrap();

        (request, response)
    }

    /// Builds an event from the provided serializable request and status code using an
    /// empty response body.
    ///
    /// # Panics
    ///
    /// Will panic if failing to convert `request` to `SdkBody`
    pub fn build_empty_event<T>(request: &T, status_code: StatusCode) -> RequestResponsePair
    where
        T: ?Sized + Serialize,
    {
        let request = Request::new(SdkBody::from(serde_json::to_string(&request).unwrap()));

        let response = Response::builder()
            .status(status_code)
            .body(SdkBody::empty())
            .unwrap();

        (request, response)
    }
}
