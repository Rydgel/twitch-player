extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate num_cpus;
extern crate url;

use self::futures::sync::mpsc;
use self::hyper::{Client, client::HttpConnector};
use self::hyper_tls::{HttpsConnector, Error as TlsError};

use super::asio::Handle;
use super::futures::*;

use std::collections::HashMap;

pub type HttpsClient = Client<HttpsConnector<HttpConnector>>;
pub type QueryParams = HashMap<String, String>;
pub type ResponseSink = mpsc::Sender<Result<hyper::Chunk, hyper::Error>>;

pub fn http_client(handle: &Handle) -> Result<HttpsClient, TlsError> {
    let connector = HttpsConnector::new(num_cpus::get(), handle)?;

    let client = Client::configure()
        .connector(connector)
        .build(handle);

    Ok(client)
}

pub fn parse_query_params(query: &str) -> QueryParams {
    use self::url::form_urlencoded::parse as parse_query;

    parse_query(query.as_bytes())
        .map(|(k, v)| (String::from(k), String::from(v)))
        .collect()
}

pub fn fetch(client: &HttpsClient, request: hyper::Request)
    -> impl Future<Item = hyper::Chunk, Error = HttpError>
{
    client.request(request)
        .map_err(HttpError::NetworkError)
        .and_then(read_response)
}

pub fn fetch_streamed(client: &HttpsClient, request: hyper::Request)
    -> impl Stream<Item = hyper::Chunk, Error = HttpError>
{
    client.request(request)
        .into_stream()
        .map_err(HttpError::NetworkError)
        .map(stream_response)
        .flatten()
}

pub fn streaming_response() -> (ResponseSink, hyper::Response) {
    let (sink, body) = hyper::Body::pair();
    let response = hyper::Response::new()
        .with_body(body);

    (sink, response)
}

fn read_response(response: hyper::Response)
    -> Box<Future<Item = hyper::Chunk, Error = HttpError>>
{
    match response.status() {
        hyper::StatusCode::Ok => {
            let full_body = response.body()
                .concat2()
                .map_err(HttpError::NetworkError);
            Box::new(full_body)
        },
        status => {
            let error = future::err(HttpError::BadStatus(status));
            Box::new(error)
        }
    }
}

fn stream_response(response: hyper::Response)
    -> Box<Stream<Item = hyper::Chunk, Error = HttpError>>
{
    match response.status() {
        hyper::StatusCode::Ok => {
            Box::new(response.body().map_err(HttpError::NetworkError))
        },
        status => {
            let error = future::err(HttpError::BadStatus(status));
            Box::new(error.into_stream())
        }
    }
}

#[derive(Debug)]
pub enum HttpError {
    NetworkError(hyper::Error),
    BadStatus(hyper::StatusCode),
}

use std::{error, fmt};

impl error::Error for HttpError {
    fn description(&self) -> &str { "Http Error" }
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
