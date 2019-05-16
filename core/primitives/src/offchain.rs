// Copyright 2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Offchain workers types

use rstd::prelude::{Vec, Box};

/// Opaque type for offchain http requests.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct HttpRequestId(pub u16);

/// Status of the HTTP request
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum HttpRequestStatus {
	/// Deadline was reached why we waited for this request to finish.
	DeadlineReached,
	/// Request timed out.
	Timeout,
	/// Request status of this ID is not known.
	Unknown,
	/// The request is finished with given status code.
	Finished(u16),
}

impl HttpRequestStatus {
	/// Parse u32 as `RequestStatus`.
	///
	/// The first hundred of codes indicate internal states.
	/// The rest are http response status codes.
	pub fn from_u32(status: u32) -> Option<Self> {
		match status {
			0 => Some(HttpRequestStatus::Unknown),
			10 => Some(HttpRequestStatus::DeadlineReached),
			20 => Some(HttpRequestStatus::Timeout),
			100...999 => Some(HttpRequestStatus::Finished(status as u16)),
			_ => None,
		}
	}

	/// Convert the status into `u32`.
	///
	/// This is an oposite conversion to `from_u32`
	pub fn as_u32(&self) -> u32 {
		match *self {
			HttpRequestStatus::Unknown => 0,
			HttpRequestStatus::DeadlineReached => 10,
			HttpRequestStatus::Timeout => 20,
			HttpRequestStatus::Finished(code) => code as u32,
		}
	}
}

/// Opaque timestamp type
#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Timestamp(u64);

/// Duration type
#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Duration(u64);

impl Duration {
	/// Create new duration representing given number of milliseconds.
	pub fn from_millis(millis: u64) -> Self {
		Duration(millis)
	}

	/// Returns number of milliseconds this Duration represents.
	pub fn millis(&self) -> u64 {
		self.0
	}
}

impl Timestamp {
	/// Creates new `Timestamp` given unix timestamp in miliseconds.
	pub fn from_unix_millis(millis: u64) -> Self {
		Timestamp(millis)
	}

	/// Increase the timestamp by given `Duration`.
	pub fn add(&self, duration: Duration) -> Timestamp {
		Timestamp(self.0.saturating_add(duration.0))
	}

	/// Decrease the timestamp by given `Duration`
	pub fn sub(&self, duration: Duration) -> Timestamp {
		Timestamp(self.0.saturating_sub(duration.0))
	}

	/// Returns a saturated difference (Duration) between two Timestamps.
	pub fn diff(&self, other: &Self) -> Duration {
		Duration(self.0.saturating_sub(other.0))
	}

	/// Return number of milliseconds since UNIX epoch.
	pub fn unix_millis(&self) -> u64 {
		self.0
	}
}

/// An extended externalities for offchain workers.
pub trait Externalities {
	/// Submit transaction.
	///
	/// The transaction will end up in the pool and be propagated to others.
	fn submit_transaction(&mut self, extrinsic: Vec<u8>) -> Result<(), ()>;

	/// Sign given piece of data with current authority key.
	///
	/// Returns `None` if signing is not available.
	fn sign(&mut self, data: &[u8]) -> Option<[u8; 64]>;

	/// Returns current UNIX timestamp (in millis)
	fn timestamp(&mut self) -> Timestamp;

	/// Pause the execution until `deadline` is reached.
	fn sleep_until(&mut self, deadline: Timestamp);

	/// Returns a random seed.
	///
	/// This is a trully random non deterministic seed generated by host environment.
	/// Obviously fine in the off-chain worker context.
	fn random_seed(&mut self) -> [u8; 32];

	/// Sets a value in the local storage.
	///
	/// Note this storage is not part of the consensus, it's only accessible by
	/// offchain worker tasks running on the same machine. It IS persisted between runs.
	fn local_storage_set(&mut self, key: &[u8], value: &[u8]);

	/// Reads a value from the local storage.
	///
	/// If the value does not exist in the storage `None` will be returned.
	/// Note this storage is not part of the consensus, it's only accessible by
	/// offchain worker tasks running on the same machine. It IS persisted between runs.
	fn local_storage_read(&mut self, key: &[u8]) -> Option<Vec<u8>>;

	/// Initiaties a http request given HTTP verb and the URL.
	///
	/// Meta is a future-reserved field containing additional, parity-codec encoded parameters.
	/// Returns the id of newly started request.
	fn http_request_start(
		&mut self,
		method: &str,
		uri: &str,
		meta: &[u8]
	) -> Result<HttpRequestId, ()>;

	/// Append header to the request.
	fn http_request_add_header(
		&mut self,
		request_id: HttpRequestId,
		name: &str,
		value: &str
	) -> Result<(), ()>;

	/// Write a chunk of request body.
	///
	/// Writing an empty chunks finalises the request.
	/// Passing `None` as deadline blocks forever.
	///
	/// Returns an error in case deadline is reached or the chunk couldn't be written.
	fn http_request_write_body(
		&mut self,
		request_id: HttpRequestId,
		chunk: &[u8],
		deadline: Option<Timestamp>
	) -> Result<(), ()>;

	/// Block and wait for the responses for given requests.
	///
	/// Returns a vector of request statuses (the len is the same as ids).
	/// Note that if deadline is not provided the method will block indefinitely,
	/// otherwise unready responses will produce `WaitTimeout` status.
	///
	/// Passing `None` as deadline blocks forever.
	fn http_response_wait(
		&mut self,
		ids: &[HttpRequestId],
		deadline: Option<Timestamp>
	) -> Vec<HttpRequestStatus>;

	/// Read all response headers.
	///
	/// Returns a vector of pairs `(HeaderKey, HeaderValue)`.
	fn http_response_headers(
		&mut self,
		request_id: HttpRequestId
	) -> Vec<(Vec<u8>, Vec<u8>)>;

	/// Read a chunk of body response to given buffer.
	///
	/// Returns the number of bytes written or an error in case a deadline
	/// is reached or server closed the connection.
	/// Passing `None` as a deadline blocks forever.
	fn http_response_read_body(
		&mut self,
		request_id: HttpRequestId,
		buffer: &mut [u8],
		deadline: Option<Timestamp>
	) -> Result<usize, ()>;

}
impl<T: Externalities + ?Sized> Externalities for Box<T> {
	fn submit_transaction(&mut self, ex: Vec<u8>) -> Result<(), ()> {
		(&mut **self).submit_transaction(ex)
	}

	fn sign(&mut self, data: &[u8]) -> Option<[u8; 64]> {
		(&mut **self).sign(data)
	}

	fn timestamp(&mut self) -> Timestamp {
		(&mut **self).timestamp()
	}

	fn sleep_until(&mut self, deadline: Timestamp) {
		(&mut **self).sleep_until(deadline)
	}

	fn random_seed(&mut self) -> [u8; 32] {
		(&mut **self).random_seed()
	}

	fn local_storage_set(&mut self, key: &[u8], value: &[u8]) {
		(&mut **self).local_storage_set(key, value)
	}


	fn local_storage_read(&mut self, key: &[u8]) -> Option<Vec<u8>> {
		(&mut **self).local_storage_read(key)
	}

	fn http_request_start(&mut self, method: &str, uri: &str, meta: &[u8]) -> Result<HttpRequestId, ()> {
		(&mut **self).http_request_start(method, uri, meta)
	}

	fn http_request_add_header(&mut self, request_id: HttpRequestId, name: &str, value: &str) -> Result<(), ()> {
		(&mut **self).http_request_add_header(request_id, name, value)
	}

	fn http_request_write_body(
		&mut self,
		request_id: HttpRequestId,
		chunk: &[u8],
		deadline: Option<Timestamp>
	) -> Result<(), ()> {
		(&mut **self).http_request_write_body(request_id, chunk, deadline)
	}

	fn http_response_wait(&mut self, ids: &[HttpRequestId], deadline: Option<Timestamp>) -> Vec<HttpRequestStatus> {
		(&mut **self).http_response_wait(ids, deadline)
	}

	fn http_response_headers(&mut self, request_id: HttpRequestId) -> Vec<(Vec<u8>, Vec<u8>)> {
		(&mut **self).http_response_headers(request_id)
	}

	fn http_response_read_body(
		&mut self,
		request_id: HttpRequestId,
		buffer: &mut [u8],
		deadline: Option<Timestamp>
	) -> Result<usize, ()> {
		(&mut **self).http_response_read_body(request_id, buffer, deadline)
	}
}


#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn timestamp_ops() {
		let t = Timestamp(5);
		assert_eq!(t.add(Duration::from_millis(10)), Timestamp(15));
		assert_eq!(t.sub(Duration::from_millis(10)), Timestamp(0));
		assert_eq!(t.diff(&Timestamp(3)), Duration(2));
	}
}