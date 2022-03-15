mod request;

use poem::{
    http::{HeaderValue, StatusCode},
    Error, IntoResponse,
};
use poem_openapi::{
    payload::{Json, PlainText},
    registry::{MetaMediaType, MetaResponse, MetaResponses, MetaSchema, MetaSchemaRef},
    types::{ToJSON, Type},
    ApiResponse, Object,
};
use serde_json::Value;

#[derive(Object)]
struct BadRequestResult {
    error_code: i32,
    message: String,
}

#[derive(ApiResponse)]
enum MyResponse {
    /// Ok
    #[oai(status = 200)]
    Ok,
    /// A
    /// B
    ///
    /// C
    #[oai(status = 400)]
    BadRequest(Json<BadRequestResult>),
    Default(StatusCode, PlainText<String>),
}

#[test]
fn meta() {
    assert_eq!(
        MyResponse::meta(),
        MetaResponses {
            responses: vec![
                MetaResponse {
                    description: "Ok",
                    status: Some(200),
                    content: vec![],
                    headers: vec![]
                },
                MetaResponse {
                    description: "A\nB\n\nC",
                    status: Some(400),
                    content: vec![MetaMediaType {
                        content_type: "application/json",
                        schema: MetaSchemaRef::Reference("BadRequestResult")
                    }],
                    headers: vec![]
                },
                MetaResponse {
                    description: "",
                    status: None,
                    content: vec![MetaMediaType {
                        content_type: "text/plain",
                        schema: MetaSchemaRef::Inline(Box::new(MetaSchema::new("string"))),
                    }],
                    headers: vec![]
                }
            ],
        },
    );
}

#[tokio::test]
async fn into_response() {
    let resp = MyResponse::Ok.into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    let mut resp = MyResponse::BadRequest(Json(BadRequestResult {
        error_code: 123,
        message: "abc".to_string(),
    }))
    .into_response();
    assert_eq!(
        serde_json::from_slice::<Value>(&resp.take_body().into_bytes().await.unwrap()).unwrap(),
        serde_json::json!({
            "error_code": 123,
            "message": "abc",
        })
    );
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let mut resp = MyResponse::Default(StatusCode::BAD_GATEWAY, PlainText("abcdef".to_string()))
        .into_response();
    assert_eq!(resp.take_body().into_string().await.unwrap(), "abcdef");
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn headers() {
    #[derive(ApiResponse)]
    enum MyResponse {
        #[oai(status = 200)]
        A,
        #[oai(status = 200)]
        B(
            /// header1
            #[oai(header = "MY-HEADER1")]
            i32,
            #[oai(header = "MY-HEADER2")] Option<String>,
        ),
        #[oai(status = 400)]
        C(
            Json<BadRequestResult>,
            #[oai(header = "MY-HEADER1")] i32,
            #[oai(header = "MY-HEADER2")] String,
        ),
        D(
            StatusCode,
            PlainText<String>,
            #[oai(header = "MY-HEADER1")] i32,
            #[oai(header = "MY-HEADER2")] String,
        ),
    }

    let meta: MetaResponses = MyResponse::meta();
    assert_eq!(meta.responses[0].headers, &[]);

    let header1 = &meta.responses[1].headers[0];
    let header2 = &meta.responses[1].headers[1];

    assert_eq!(header1.name, "MY-HEADER1");
    assert_eq!(header1.description.as_deref(), Some("header1"));
    assert_eq!(header1.required, true);
    assert_eq!(
        header1.schema,
        MetaSchemaRef::Inline(Box::new(MetaSchema::new_with_format("integer", "int32")))
    );

    assert_eq!(header2.name, "MY-HEADER2");
    assert_eq!(header2.description, None);
    assert_eq!(header2.required, false);
    assert_eq!(
        header2.schema,
        MetaSchemaRef::Inline(Box::new(MetaSchema::new("string")))
    );

    let resp = MyResponse::A.into_response();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = MyResponse::B(88, Some("abc".to_string())).into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("MY-HEADER1"),
        Some(&HeaderValue::from_static("88"))
    );
    assert_eq!(
        resp.headers().get("MY-HEADER2"),
        Some(&HeaderValue::from_static("abc"))
    );

    let resp = MyResponse::B(88, None).into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("MY-HEADER1"),
        Some(&HeaderValue::from_static("88"))
    );
    assert!(!resp.headers().contains_key("MY-HEADER2"));

    let mut resp = MyResponse::C(
        Json(BadRequestResult {
            error_code: 11,
            message: "hehe".to_string(),
        }),
        88,
        "abc".to_string(),
    )
    .into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        serde_json::from_slice::<Value>(&resp.take_body().into_bytes().await.unwrap()).unwrap(),
        serde_json::json!({
            "error_code": 11,
            "message": "hehe",
        })
    );
    assert_eq!(
        resp.headers().get("MY-HEADER1"),
        Some(&HeaderValue::from_static("88"))
    );
    assert_eq!(
        resp.headers().get("MY-HEADER2"),
        Some(&HeaderValue::from_static("abc"))
    );

    let mut resp = MyResponse::D(
        StatusCode::CONFLICT,
        PlainText("abcdef".to_string()),
        88,
        "abc".to_string(),
    )
    .into_response();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_eq!(resp.take_body().into_string().await.unwrap(), "abcdef");
    assert_eq!(
        resp.headers().get("MY-HEADER1"),
        Some(&HeaderValue::from_static("88"))
    );
    assert_eq!(
        resp.headers().get("MY-HEADER2"),
        Some(&HeaderValue::from_static("abc"))
    );
}

#[tokio::test]
async fn bad_request_handler() {
    #[derive(ApiResponse, Debug, Eq, PartialEq)]
    #[oai(bad_request_handler = "bad_request_handler")]
    #[allow(dead_code)]
    pub enum CustomApiResponse {
        #[oai(status = 200)]
        Ok,
        #[oai(status = 400)]
        BadRequest,
    }

    fn bad_request_handler(_: Error) -> CustomApiResponse {
        CustomApiResponse::BadRequest
    }

    assert_eq!(
        CustomApiResponse::from_parse_request_error(Error::from_status(StatusCode::BAD_GATEWAY)),
        CustomApiResponse::BadRequest
    );
}

#[tokio::test]
async fn generic() {
    #[derive(ApiResponse)]
    pub enum CustomApiResponse<T: ToJSON> {
        #[oai(status = 200)]
        Ok(Json<T>),
    }

    assert_eq!(
        CustomApiResponse::<String>::meta(),
        MetaResponses {
            responses: vec![MetaResponse {
                description: "",
                status: Some(200),
                content: vec![MetaMediaType {
                    content_type: "application/json",
                    schema: MetaSchemaRef::Inline(Box::new(MetaSchema::new("string")))
                }],
                headers: vec![]
            },],
        },
    );

    let mut resp = CustomApiResponse::Ok(Json("success".to_string())).into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<Value>(&resp.take_body().into_bytes().await.unwrap()).unwrap(),
        serde_json::json!("success")
    );
}

#[tokio::test]
async fn item_content_type() {
    #[derive(ApiResponse, Debug, Eq, PartialEq)]
    pub enum Resp {
        #[oai(status = 200, content_type = "application/json2")]
        A(Json<i32>),
        #[oai(content_type = "application/json3")]
        B(StatusCode, Json<i32>),
    }

    assert_eq!(
        Resp::meta(),
        MetaResponses {
            responses: vec![
                MetaResponse {
                    description: "",
                    status: Some(200),
                    content: vec![MetaMediaType {
                        content_type: "application/json2",
                        schema: MetaSchemaRef::Inline(Box::new(MetaSchema::new_with_format(
                            "integer", "int32"
                        )))
                    }],
                    headers: vec![]
                },
                MetaResponse {
                    description: "",
                    status: None,
                    content: vec![MetaMediaType {
                        content_type: "application/json3",
                        schema: MetaSchemaRef::Inline(Box::new(MetaSchema::new_with_format(
                            "integer", "int32"
                        )))
                    }],
                    headers: vec![]
                }
            ],
        },
    );

    let mut resp = Resp::A(Json(100)).into_response();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.content_type(), Some("application/json2"));
    assert_eq!(
        serde_json::from_slice::<Value>(&resp.take_body().into_bytes().await.unwrap()).unwrap(),
        serde_json::json!(100)
    );

    let mut resp = Resp::B(StatusCode::BAD_GATEWAY, Json(200)).into_response();
    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(resp.content_type(), Some("application/json3"));
    assert_eq!(
        serde_json::from_slice::<Value>(&resp.take_body().into_bytes().await.unwrap()).unwrap(),
        serde_json::json!(200)
    );
}

#[tokio::test]
async fn header_deprecated() {
    #[derive(ApiResponse, Debug, Eq, PartialEq)]
    #[allow(dead_code)]
    pub enum Resp {
        #[oai(status = 200)]
        A(Json<i32>, #[oai(header = "A", deprecated = true)] String),
    }

    let meta: MetaResponses = Resp::meta();
    assert_eq!(meta.responses[0].headers[0].deprecated, true);
}

#[tokio::test]
async fn extra_headers_on_response() {
    #[derive(ApiResponse, Debug, Eq, PartialEq)]
    #[oai(
        header(name = "A1", type = "String"),
        header(name = "a2", type = "i32", description = "abc", deprecated = true)
    )]
    #[allow(dead_code)]
    pub enum Resp {
        #[oai(status = 200)]
        A(Json<i32>, #[oai(header = "A")] String),
    }

    let meta: MetaResponses = Resp::meta();
    assert_eq!(meta.responses[0].headers.len(), 3);

    assert_eq!(meta.responses[0].headers[0].name, "A");
    assert_eq!(meta.responses[0].headers[0].deprecated, false);

    assert_eq!(meta.responses[0].headers[1].name, "A1");
    assert_eq!(meta.responses[0].headers[1].description, None);
    assert_eq!(meta.responses[0].headers[1].deprecated, false);
    assert_eq!(meta.responses[0].headers[1].schema, String::schema_ref());

    assert_eq!(meta.responses[0].headers[2].name, "A2");
    assert_eq!(
        meta.responses[0].headers[2].description.as_deref(),
        Some("abc")
    );
    assert_eq!(meta.responses[0].headers[2].deprecated, true);
    assert_eq!(meta.responses[0].headers[2].schema, i32::schema_ref());
}

#[tokio::test]
async fn extra_headers_on_item() {
    #[derive(ApiResponse, Debug, Eq, PartialEq)]
    #[allow(dead_code)]
    pub enum Resp {
        #[oai(
            status = 200,
            header(name = "A1", type = "String"),
            header(name = "a2", type = "i32", description = "abc", deprecated = true)
        )]
        A(Json<i32>, #[oai(header = "A")] String),
    }

    let meta: MetaResponses = Resp::meta();
    assert_eq!(meta.responses[0].headers.len(), 3);

    assert_eq!(meta.responses[0].headers[0].name, "A");
    assert_eq!(meta.responses[0].headers[0].deprecated, false);

    assert_eq!(meta.responses[0].headers[1].name, "A1");
    assert_eq!(meta.responses[0].headers[1].description, None);
    assert_eq!(meta.responses[0].headers[1].deprecated, false);

    assert_eq!(meta.responses[0].headers[2].name, "A2");
    assert_eq!(
        meta.responses[0].headers[2].description.as_deref(),
        Some("abc")
    );
    assert_eq!(meta.responses[0].headers[2].deprecated, true);
}
