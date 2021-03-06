use {Response};
use error::Error;
use generic::{Encoder, Encode};

use {http, h2};
use futures::{Future, Stream, Poll, Async};
use http::header;

#[derive(Debug)]
pub struct ResponseFuture<T, E> {
    inner: T,
    encoder: Option<E>,
}

// ===== impl ResponseFuture =====

impl<T, E, S> ResponseFuture<T, E>
where T: Future<Item = Response<S>,
               Error = ::Status>,
      E: Encoder,
      S: Stream<Item = E::Item>,
{
    pub fn new(inner: T, encoder: E) -> Self {
        ResponseFuture {
            inner,
            encoder: Some(encoder),
        }
    }
}

impl<T, E, S> Future for ResponseFuture<T, E>
where T: Future<Item = Response<S>,
               Error = ::Status>,
      E: Encoder,
      S: Stream<Item = E::Item>,
      S::Error: Into<Error>,
{
    type Item = http::Response<Encode<E, S>>;
    type Error = h2::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // Get the gRPC response
        let response = match self.inner.poll() {
            Ok(Async::Ready(response)) => response,
            Ok(Async::NotReady) => return Ok(Async::NotReady),
            Err(status) => {
                let response = Response::new(Encode::error(status));
                return Ok(response.into_http().into());
            }
        };

        // Convert to an HTTP response
        let mut response = response.into_http();
        // Set the content type
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static(E::CONTENT_TYPE),
        );

        // Get the encoder
        let encoder = self.encoder.take().expect("encoder consumed");

        // Map the response body
        let response = response.map(move |body| {
            Encode::response(encoder, body)
        });

        Ok(response.into())
    }
}
