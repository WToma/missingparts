//! Some helper types and functions to work with requests and responses in hyper with serde
//!
//! See the [`RichParts`](struct.RichParts.html) type for useful methods and a complete example.

use std::convert::TryFrom;
use std::fmt;
use std::str;

use http::request::Parts;

use hyper::header::{HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE};
use hyper::{Body, Error as HyperError, Response, StatusCode};

use json5;
use serde::{de, Serialize};
use serde_json;

/// The different errors that can happen while parsing a request body.
///
/// The enum can be directly turned into a response using `into::<Response<Body>>()`.
pub enum BodyParseError {
    /// The request specified an unsupported Content-Type for the request body. The unsupported content type is
    /// indicated inside the error.
    ///
    /// Note: the Content-Type header is optional, if missing, a default content type will be assumed. (See
    /// [`RichRequest.get_content_type`](struct.RichRequest.html#method.get_content_type).) However if present it
    /// _must_ be one of the supported content types, otherwise this error will happen.
    UnsupportedContentType(MimeType),

    /// The request specified an unsupported charset attribute inside the Content-Type header of the request. The
    /// unsupported charset is indicated inside the error.
    ///
    /// Note that specifying the charset attribute of the request is optional. If missing, a utf8 will be assumed. All
    /// other charsets are unsupported.
    UnsupportedCharset(String),

    /// The Content-Length of the request was larger than the maximum the server was willing to process. (This is the
    /// `max_length` parameter specified to the deserialization method.)
    RequestTooLarge {
        /// The maximum size the server is willing to process
        max_size: usize,

        /// The size of the request
        actual_size: usize,
    },

    /// Errors encountered by the server framework while trying to read the whole request body into memory. Lots of
    /// these indicate errors around prematurely closed client connections, so these are not necessarily server errors
    /// per se.
    BodyReadingError(HyperError),

    /// The request did not specify the required Content-Length header.
    ContentLengthMissing,

    /// The request body could not be read as UTF8-encoded text. Note that only UTF8 is supported.
    EncodingError(str::Utf8Error),

    /// The request could not be parsed as the specified JSON object. Note that if Content-Type was not specified, JSON
    /// is assumed.
    JsonError(serde_json::Error),

    /// The request could not be parsed as the specified JSON5 object.
    Json5Error(json5::Error),
}

impl From<HyperError> for BodyParseError {
    fn from(e: HyperError) -> BodyParseError {
        BodyParseError::BodyReadingError(e)
    }
}
impl From<str::Utf8Error> for BodyParseError {
    fn from(e: str::Utf8Error) -> BodyParseError {
        BodyParseError::EncodingError(e)
    }
}
impl From<serde_json::Error> for BodyParseError {
    fn from(e: serde_json::Error) -> BodyParseError {
        BodyParseError::JsonError(e)
    }
}
impl From<json5::Error> for BodyParseError {
    fn from(e: json5::Error) -> BodyParseError {
        BodyParseError::Json5Error(e)
    }
}
impl Into<Response<Body>> for BodyParseError {
    fn into(self) -> Response<Body> {
        use BodyParseError::*;
        let (code, message) = match self {
            UnsupportedContentType(content_type) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "unsupported content type '{}'. Try 'application/json'",
                    content_type
                ),
            ),
            UnsupportedCharset(charset) => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "unsupported charset '{}'. the request body must be charset=utf8",
                    charset
                ),
            ),
            RequestTooLarge {
                actual_size,
                max_size,
            } => (
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "body must be not larger than {} bytes, was {}",
                    max_size, actual_size
                ),
            ),
            ContentLengthMissing => (
                StatusCode::LENGTH_REQUIRED,
                String::from("Content-Length header must be specified"),
            ),
            BodyReadingError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from("failed to read the request body"),
            ),
            EncodingError(utf8_error) => (
                StatusCode::BAD_REQUEST,
                format!(
                    "the request body was not parseable as UTF-8 at position {}",
                    utf8_error.valid_up_to()
                ),
            ),
            JsonError(e) => (
                StatusCode::BAD_REQUEST,
                format!("could not parse the json request: {}", e),
            ),
            Json5Error(e) => (
                StatusCode::BAD_REQUEST,
                format!("could not parse the json5 request: {}", e),
            ),
        };
        Response::builder()
            .status(code)
            .body(Body::from(message))
            .unwrap()
    }
}

/// Simple helpers to parse a content type string that may also contain a charset
struct ContentType {
    content_type: MimeType,
    charset_name: Option<String>,
}
impl From<&str> for ContentType {
    fn from(s: &str) -> ContentType {
        let mut parts = s.split(';');
        let content_type = MimeType::from(parts.next().unwrap()); // the first part is always present

        let charset_name = parts
            .map(|s| s.trim())
            .filter(|s| s.starts_with("charset="))
            .next()
            .map(|s| String::from(&s[8..]));
        ContentType {
            content_type,
            charset_name,
        }
    }
}
impl From<MimeType> for ContentType {
    fn from(m: MimeType) -> ContentType {
        ContentType {
            content_type: m,
            charset_name: None,
        }
    }
}

/// A MIME type and subtype. Both the type and the subtype can be `*` (wildcard). This is currently only useful
/// as an intermediate type for [`SupportedMimeType`](enum.SupportedMimeType.html), and to denote the unsupported
/// MIME type in [`BodyParseError::UnsupportedContentType`](enum.BodyParseError.html#variant.UnsupportedContentType).
///
/// To create an instance, use `MimeType::from(&str)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MimeType {
    mime_type: String,
    mime_subtype: String,
}
impl MimeType {
    fn is_compatible_with(&self, other: &MimeType) -> bool {
        Self::is_compatible_part(&self.mime_type, &other.mime_type)
            && Self::is_compatible_part(&self.mime_subtype, &other.mime_subtype)
    }

    fn is_compatible_part(part1: &str, part2: &str) -> bool {
        part1 == "*" || part2 == "*" || part1 == part2
    }

    fn json() -> MimeType {
        MimeType::from("application/json")
    }
    fn json5() -> MimeType {
        MimeType::from("application/json5")
    }
}
impl From<&str> for MimeType {
    /// Parses a slash-separated mime type and subtype.
    ///
    /// # Examples
    ///
    /// ```
    /// # use missingparts::server_utils::MimeType;
    /// # use std::convert::From;
    ///
    /// MimeType::from("application/json");
    /// MimeType::from("application/*");
    /// ```
    fn from(s: &str) -> MimeType {
        let mut parts = s.split('/');
        let mime_type = String::from(parts.next().unwrap()); // OK to unwrap, first element on spliterator always exists
        let mime_subtype = String::from(parts.next().unwrap_or("*"));
        MimeType {
            mime_type,
            mime_subtype,
        }
    }
}
impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.mime_type, self.mime_subtype)
    }
}

/// A MIME type that can be used for serializing and deserializing content. Currently JSON and JSON5 are supported.
///
/// This can be used to serialize a response body using [`serialize`](#method.serialize).
pub enum SupportedMimeType {
    Json,
    Json5,
}
impl SupportedMimeType {
    fn from_mime_type(m: &MimeType) -> Result<SupportedMimeType, &MimeType> {
        use SupportedMimeType::*;
        match m {
            t if t == &MimeType::json() => Ok(Json),
            t if t == &MimeType::json5() => Ok(Json5),
            unsupported => Err(unsupported),
        }
    }

    fn from_mime_type_relaxed(m: &MimeType) -> Result<SupportedMimeType, &MimeType> {
        use SupportedMimeType::*;
        match m {
            t if t.is_compatible_with(&MimeType::json()) => Ok(Json),
            t if t.is_compatible_with(&MimeType::json5()) => Ok(Json5),
            unsupported => Err(unsupported),
        }
    }

    /// Serializes `x` according to this supported mime type into a hyper response body.
    pub fn serialize<T: Serialize>(&self, x: &T) -> Body {
        use SupportedMimeType::*;
        match self {
            Json => Body::from(serde_json::ser::to_string(x).unwrap()),
            Json5 => Body::from(json5::to_string(x).unwrap()),
        }
    }

    fn deserialize<T: de::DeserializeOwned>(&self, s: &str) -> Result<T, BodyParseError> {
        use SupportedMimeType::*;
        match self {
            Json => Ok(serde_json::de::from_str(s)?),
            Json5 => Ok(json5::from_str(s)?),
        }
    }
}

/// Helper to work with the Accept http headers
struct Accept {
    mime_types: Vec<MimeType>,
}
impl Accept {
    fn is_empty(&self) -> bool {
        self.mime_types.is_empty()
    }
}
impl From<&str> for Accept {
    fn from(s: &str) -> Accept {
        Accept {
            mime_types: s
                .split(',')
                .map(|part| part.split(";").next().unwrap()) // unwrap is OK because at least 1 part will always exist
                .map(|s| MimeType::from(s))
                .collect(),
        }
    }
}
impl<'a> From<hyper::header::GetAll<'a, HeaderValue>> for Accept {
    fn from(headers: hyper::header::GetAll<'a, HeaderValue>) -> Accept {
        let accepts = headers
            .iter()
            .filter_map(|h| h.to_str().ok())
            .map(Accept::from);
        let mut all_mime_types: Vec<MimeType> = Vec::new();
        for mut accept in accepts {
            all_mime_types.append(&mut accept.mime_types);
        }
        Accept {
            mime_types: all_mime_types,
        }
    }
}

/// Provides convenience methods to work with requests.
///
/// To deserialize a request, use [`deserialize_by_content_type`](#method.deserialize_by_content_type). Conversely,
/// to serialize a response use [`guess_response_type`](#method.guess_response_type) and use
/// [`serialize`](enum.SupportedMimeType.html#method.serialize) on the result. For serialization and deserialization
/// currently JSON (`application/json`) and JSON5 (`application/json5`) are supported.
///
/// To see if the request matches a particular path, see [`try_match`](#method.try_match) and
/// [`does_match`](#method.does_match).
///
/// The requests that can be used with `RichParts` have a few restrictions:
/// - The `Content-Length` header must be present if the request body needs to be parsed.
/// - For parsing the request body, the `Content-Type` header must be either present and have one of the supported
///   values, or if missing, `application/json` will be assumed, so the request body must be valid JSON.
/// - The request body must be valid UTF-8. No other encodings are supported.
/// - If a response is to be returned, either an `Accept` header is required with one of the supported content types,
///   or the response will default to the same MIME type as `Content-Type`.
///
/// Quickly access the Authorization header value with [`token`](#method.token).
///
/// # Examples
///
/// ```
/// # use missingparts::server_utils::*;
/// # use hyper::{Request, Response, Body};
/// # use serde::{Deserialize, Serialize};
/// # use http;
/// # use tokio::runtime::Runtime;
/// # use core::future::Future;
/// # use std::str;
/// # fn get_response_text<R>(r: R) -> String where R: Future<Output=Response<Body>> {
/// #     let full_body_future = async {
/// #       let r = r.await;
/// #       let body = r.into_body();
/// #       hyper::body::to_bytes(body).await
/// #     };
/// #     let full_body = Runtime::new()
/// #         .unwrap()
/// #         .block_on(full_body_future)
/// #         .unwrap();
/// #     let full_body_str = str::from_utf8(&full_body).unwrap();
/// #     String::from(full_body_str)
/// # }
/// #[derive(Deserialize)]
/// struct MyImportantRequest {
///     my_important_field: i32,
/// }
///
/// #[derive(Serialize)]
/// struct MyImportantResponse {
///     requester_id: u32,
///     message: String,
/// }
///
/// let body = "{my_important_field: 42}";
///
/// // create a simple mock request with a JSON5 request body, expecting a JSON response.
/// // this is not very typical, but it's possible with `RichParts`
/// let request = Request::post("/echo/1234")
///     .header("Content-Type", "application/json5")
///     .header("Accept", "application/json")
///     .header("Content-Length", &format!("{:?}", body.len()))
///     .body(Body::from(body)).unwrap();
///
/// let response = async {
///     let (parts, body) = request.into_parts();
///     let rich_parts = RichParts::from(&parts);
///
///     // in real code reply with NOT_ACCEPTABLE here
///     let response_mime_type = rich_parts.guess_response_type().unwrap();
///
///     // this mock service only handles one path, that has one path parameter
///     // if the request method and request path match the path pattern, then try to deserialize
///     // the request into a struct.
///     if let Ok(TupleWrapper1(requester_id)) = rich_parts.try_match(
///         &http::Method::POST,
///         "/echo/{}",
///     ) {
///         let request: MyImportantRequest = rich_parts.deserialize_by_content_type(body, 1024)
///             .await
///             // in real code turn the error value into a HTTP response using `into::Response<<Body>>()`
///             .unwrap_or_else(|_| panic!("failed to parse request body"));
///         Response::builder()
///             .status(http::StatusCode::OK)
///             .body(response_mime_type.serialize(&MyImportantResponse {
///                 requester_id,
///                 message: format!("your important value is {}", request.my_important_field),
///             }))
///             .unwrap()
///     } else {
///         panic!("wrong path"); // in real code reply with NOT_FOUND or METHOD_NOT_ALLOWED
///     }
/// };
///
/// assert_eq!(
///     get_response_text(response),
///     "{\"requester_id\":1234,\"message\":\"your important value is 42\"}",
/// );
/// ```
pub struct RichParts {
    method: http::Method,
    uri_path: String,
    content_type: ContentType,
    content_length: Option<usize>,
    accept: Accept,
    token: Option<String>,
}
impl From<&Parts> for RichParts {
    fn from(parts: &Parts) -> RichParts {
        let method = parts.method.clone();
        let uri_path = parts.uri.path().to_string();
        let content_type = Self::parse_content_type(parts);
        let content_length = Self::parse_content_length(parts);
        let accept = Self::parse_accept(parts);
        let token = Self::parse_token(parts);
        RichParts {
            method,
            uri_path,
            content_type: content_type,
            content_length,
            accept,
            token,
        }
    }
}
impl RichParts {
    fn get_content_type(&self) -> &ContentType {
        &self.content_type
    }

    fn get_content_length(&self) -> &Option<usize> {
        &self.content_length
    }

    /// Returns a best effort value as to what the content type of the response should be.
    ///
    /// The rules to determine the response content type are the following:
    /// 1. If the client specified a supported MIME type in the Accept header, one of those values will be used (note:
    ///    currently weights are not supported, instead the first supported value will be used).
    /// 2. If the caller sent an Accept header, but it contained no supported MIME types, the result will be `None`.
    /// 3. Otherwise, if the Content-Type is known and supported, that will be returned. (This includes falling back to
    ///    the default content type.)
    pub fn guess_response_type(&self) -> Option<SupportedMimeType> {
        let explicit_accept_type = self
            .accept
            .mime_types
            .iter()
            .filter_map(|explicit_accept_type| {
                SupportedMimeType::from_mime_type_relaxed(explicit_accept_type).ok()
            })
            .next();
        // if the caller specified a supported accepy type, use that
        explicit_accept_type.or_else(|| {
            if !self.accept.is_empty() {
                // if the caller specified accept types, and none of them were supported, then do not attempt to
                // guess the response type
                None
            } else {
                // if the Content-Type was specified and supported, use that. Otherwise, fall back to the default.
                // `content_type` already does the default handling, so we don't have to repeat that here
                // (if the content type was specified and unsupported, we will likely not use the result of this
                // funciton anyway).
                SupportedMimeType::from_mime_type(&self.content_type.content_type).ok()
            }
        })
    }

    /// Deserialies the request body into the specified type.
    ///
    /// The request must be in the format specified by `Content-Type` (see the struct-level documentation). Failing to
    /// read or parse the body according to the expected `Content-Type`, and `T`, will cause this method to return an
    /// error. The error can be directly turned into an appropriate HTTP response, see
    /// [`BodyParseError`](enum.BodyParseError.html).
    ///
    /// To protect against requests that are too large, a possible attack vector, a maximum size must be specified. If
    /// the content length is greater than `max_content_length`, the function will not make an attempt to read the body.
    /// Hyper internally ensures that the body is not read eagerly before requested, and it will not read more than the
    /// number of bytes in `Content-Length` (if present). Thus this method requires `Content-Length` to be present and
    /// not greater than `max_content_length`.
    pub async fn deserialize_by_content_type<T: de::DeserializeOwned>(
        &self,
        body: Body,
        max_content_length: usize,
    ) -> Result<T, BodyParseError> {
        use BodyParseError::*;
        let content_type = self.get_content_type();

        // it is allowed for the Content-Type charset to be left unspecified. in that case we assume utf8. Values
        // other than utf8 are not allowed.
        match &content_type.charset_name.as_ref().filter(|c| c != &"utf8") {
            Some(unsupported_charset) => {
                return Err(UnsupportedCharset(unsupported_charset.to_string()))
            }
            None => (),
        };

        // check that the Content-Type is supported. (assuming application/json if unspecified)
        let content_type = SupportedMimeType::from_mime_type(&content_type.content_type).map_err(
            |unsupported_mime_type| UnsupportedContentType(unsupported_mime_type.clone()),
        )?;

        // check that the Content-Length is defined (mandatory) and that it does not exceed the max size the
        // handler is willing to process (this is to prevent DoS-type attacks. hyper will limit the body input stream
        // to Content-Length bytes).
        let content_length: usize = self.get_content_length().ok_or(ContentLengthMissing)?;
        if content_length > max_content_length {
            return Err(RequestTooLarge {
                actual_size: content_length,
                max_size: max_content_length,
            });
        }

        // read & deserialize the body
        let full_body = hyper::body::to_bytes(body).await?;
        let full_body_str = str::from_utf8(&full_body)?;
        content_type.deserialize(full_body_str)
    }

    /// Returns `true` if the request method in this request is the same as `method`, and the request path is the same
    /// as `path`.
    ///
    /// If you need to match paths with a path variable (placeholder) in them, see
    /// [`try_match`](struct.RichParts.try_match).
    pub fn does_match(&self, method: &http::Method, path: &str) -> bool {
        &self.method == method && self.uri_path == path
    }

    /// Attempts to match the request method and path in this request against the specified method and path pattern.
    ///
    /// This method is intended to be used with the `TupleWrapperN` types (where `N` is the number of members in the
    /// tuple, so in this case the number of path variables, or placeholders).
    ///
    /// # Parameters
    /// - `method`: if the request method is not the same as this, this function will fail with
    ///   [`MethodNotAllowed`](enum.UriMatchError#variant.MethodNotAllowed) before even attempting to parse the
    ///   path or the path pattern.
    /// - `path_pattern` is the path pattern against which the actual request path will be matched.
    ///
    /// # Path Patterns
    /// The path pattern consists of a sequence of parts. Each part is separated by a literal `/`. Each part can be
    /// either a constant, or a variable (placeholder). The constants are literals of characters permitted in URIs,
    /// except a `/`. The variables are marked by `{}`.
    ///
    /// Some valid path patterns would be:
    /// - `/authors/{}` -- matches `/authors/123`, `/authors/tolkien`, or `/authors/j+r+r+tolkien`
    /// - `/authors/{}/books/{}/` -- matches `/authors/123/books/the+lord+of+the+rings/`
    /// - `/{}/{}/{}` -- matches `/123/metal%2Fhammer/321`
    ///
    /// Leading `/`s are required, as hyper will provide the request path with a leading `/`. Trailing `/`s are
    /// optional, in that the path pattern is valid without it, but the client and the server must agree whether they
    /// should be present or not.
    ///
    /// Valid patterns that are useless in practice
    /// - `/authors`, `/` -- there are no variables, so none of the built-in `TupleWrapperN` types can be used. You
    ///   could provide your own, but in this case just use [`does_match`](struct.RichParts.does_match).
    ///
    /// # Matching Path Patterns
    /// The request path is considered to be matching the path pattern if the following are true:
    /// - they have the same number of parts.
    /// - if the part at the `i`th place of the pattern is a constant, the same constant is present as the `i`th part
    ///   of the request path.
    /// - if the part at the `i`th place of the pattern is a variable, the text present as the `i`th part of the request
    ///   path must be parseable into the required type.
    ///  
    /// For example, the following cases would _not_ match:
    /// - request path: `/authors/Douglas+Adams/books/42`, pattern: `/players/{}/inventory` (the first constant
    ///   component did not match)
    /// - request path: `/authors/Douglas+Adams/books/42`, pattern: `/authors/{}` (the request had more components to it
    ///   than the pattern)
    ///
    /// # A note on URL encoding
    ///
    /// Note that in the above examples we used URL encoded values like `j+r+r+tolkien` or `metal%2Fhammer`. `RichParts`
    /// does not do any decoding these values; if you match on any of these, they will yield the encoded values. Thus
    /// further processing is needed to URL decode any strings you may receive.
    ///
    /// # Examples
    /// ```
    /// # use hyper::Request;
    /// # use http;
    /// #
    /// # use missingparts::server_utils::*;
    ///
    /// let request_path = "/authors/Douglas%20Adams/books/42";
    ///
    /// let request: Request<()> = Request::get(request_path).body(()).unwrap();
    /// let (parts, body) = request.into_parts();
    /// let rich_parts = RichParts::from(&parts);
    ///
    /// let expected_path = "/authors/{}/books/{}";
    ///
    /// // note: most of the time you can omit the explicit type parameters for the `TupleWrapperN`
    /// // types. The compiler will infer them from their usage. In this case we're doing ambigous
    /// // `assert_eq` macro calls only so the compiler won't be able to infer the exact types.
    /// if let Ok(TupleWrapper2::<String, u32>(author, book_id)) = rich_parts.try_match(
    ///     &http::Method::GET,
    ///     expected_path,
    /// ) {
    ///     assert_eq!(author, "Douglas%20Adams");
    ///     assert_eq!(book_id, 42);
    /// } else {
    ///     panic!(format!("'{}' did not match '{}'", request_path, expected_path));
    /// }
    /// ```
    pub fn try_match<'a, 'b, T>(
        &'a self,
        method: &http::Method,
        path_pattern: &'b str,
    ) -> Result<T, UriMatchError>
    where
        T: TryFrom<Vec<&'a str>, Error = TupleWrapperParseError>,
    {
        use UriMatchError::*;
        if &self.method != method {
            return Err(MethodNotAllowed(method.clone()));
        }

        let mut path_pattern_parts = path_pattern.split('/');
        let mut path_parts = self.uri_path[..].split('/');
        let mut variables: Vec<&'a str> = Vec::new();

        loop {
            let next_pattern_part = path_pattern_parts.next();
            let next_path_part = path_parts.next();
            match (next_pattern_part, next_path_part) {
                (None, None) => break,
                (Some("{}"), Some(variable)) => variables.push(variable),
                (Some(pattern_part), Some(path_part)) if path_part == pattern_part => continue,
                _ => return Err(PathDoesNotMatch(self.uri_path.clone())),
            }
        }

        T::try_from(variables).map_err(|e| PartsParseError(e))
    }

    /// Returns the value of the `Authorization` header, if it was present.
    pub fn token(&self) -> Option<&str> {
        self.token.as_ref().map(|s| s.as_str())
    }

    // header parsing helpers

    fn parse_content_type(parts: &Parts) -> ContentType {
        parts
            .headers
            .get(CONTENT_TYPE)
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .map(|h| ContentType::from(h))
            .unwrap_or_else(|| ContentType::from(MimeType::json()))
    }

    fn parse_content_length(parts: &Parts) -> Option<usize> {
        parts
            .headers
            .get(CONTENT_LENGTH)
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.parse().ok())
    }

    fn parse_accept(parts: &Parts) -> Accept {
        Accept::from(parts.headers.get_all(ACCEPT))
    }

    fn parse_token(parts: &Parts) -> Option<String> {
        parts
            .headers
            .get(AUTHORIZATION)
            .iter()
            .next()
            .and_then(|h| h.to_str().ok())
            .map(|h| h.to_string())
    }
}

// helpers for parsing URLs

/// The different errors that can happen while attempting to use [`try_match`](struct.RichParts.html#method.try_match).
pub enum UriMatchError {
    /// The method of the HTTP request did not match the expected method.
    ///
    /// `.0` is the actual HTTP method from the request.
    ///
    /// Note that if you receive this error, that means that `try_match` have not tried to parse the path at all, so
    /// receiving this error should not be taken as an indication that the path itself matched, and it was only the
    /// method that was mismatched.
    ///
    /// In practice, if your application processes multiple paths, the code should keep trying those other paths to
    /// see if any of them match.
    MethodNotAllowed(http::Method),

    /// The path of the HTTP request did not match the expected path pattern.
    ///
    /// `.0` is the actual path from the HTTP request.
    PathDoesNotMatch(String),

    /// The path of the HTTP request matched the pattern, but some path variables could not be parsed to the expected
    /// type.
    ///
    /// `.0` contains more details about what exactly was the problem.
    ///
    /// This can happen for example if a path parameter expected an integer, but was given a string that could not be
    /// parsed as an integer. Another possible cause of this error is if the code uses a `TupleWrapperN` type where
    /// `N != the number of path variables in the pattern`.
    PartsParseError(TupleWrapperParseError),
}

/// Detailed information about why parsing parameters for a matching path (see
/// [`PartsParseError`](enum.UriMatchError.html#variant.PartsParseError)) failed.
pub enum TupleWrapperParseError {
    /// The path pattern contained more parameters than the output tuple.
    ///
    /// This is usually a programming error, you likely have the wrong `TupleWrapperN` variant.
    TooManyParams(usize),

    /// The path pattern contained less parameters than the output tuple.
    ///
    /// This is usually a programming error, you likely have the wrong `TupleWrapperN` variant.
    TooFewParams(usize),

    /// One of the parts could not be parsed to the expected type. `.0` indicates which part failed parsing.
    FailedToParsePart(usize),
}

/// Used to parse path parameters
///
/// See [`try_match`](struct.RichParts.html#method.try_match) for an example.
pub struct TupleWrapper1<T>(pub T);
impl<'a, T1> TryFrom<Vec<&'a str>> for TupleWrapper1<T1>
where
    T1: str::FromStr,
{
    type Error = TupleWrapperParseError;

    fn try_from(s: Vec<&'a str>) -> Result<Self, Self::Error> {
        use TupleWrapperParseError::*;
        let l = s.len();
        match l {
            1 => {
                let inner = T1::from_str(s[0]).map_err(|_| FailedToParsePart(0))?;
                Ok(TupleWrapper1(inner))
            }
            too_few if l < 1 => Err(TooFewParams(too_few)),
            too_many => Err(TooManyParams(too_many)),
        }
    }
}

/// Used to parse path parameters
///
/// See [`try_match`](struct.RichParts.html#method.try_match) for an example.
pub struct TupleWrapper2<T1, T2>(pub T1, pub T2);
impl<'a, T1, T2> TryFrom<Vec<&'a str>> for TupleWrapper2<T1, T2>
where
    T1: str::FromStr,
    T2: str::FromStr,
{
    type Error = TupleWrapperParseError;

    fn try_from(s: Vec<&'a str>) -> Result<Self, Self::Error> {
        use TupleWrapperParseError::*;
        let l = s.len();
        match l {
            2 => {
                let inner1 = T1::from_str(s[0]).map_err(|_| FailedToParsePart(0))?;
                let inner2 = T2::from_str(s[1]).map_err(|_| FailedToParsePart(1))?;
                Ok(TupleWrapper2(inner1, inner2))
            }
            too_few if l < 2 => Err(TooFewParams(too_few)),
            too_many => Err(TooManyParams(too_many)),
        }
    }
}
