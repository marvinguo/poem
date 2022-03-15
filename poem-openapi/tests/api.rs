use poem::{
    http::{Method, StatusCode},
    test::TestClient,
    web::Data,
    EndpointExt, Error,
};
use poem_openapi::{
    param::Query,
    payload::{Binary, Json, PlainText},
    registry::{MetaApi, MetaExternalDocument, MetaParamIn, MetaSchema},
    types::Type,
    ApiRequest, ApiResponse, OpenApi, OpenApiService, Tags,
};

#[tokio::test]
async fn path_and_method() {
    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(path = "/abc", method = "post")]
        async fn test(&self) {}
    }

    let meta: MetaApi = Api::meta().remove(0);
    assert_eq!(meta.paths[0].path, "/abc");
    assert_eq!(meta.paths[0].operations[0].method, Method::POST);

    let ep = OpenApiService::new(Api, "test", "1.0");
    let cli = TestClient::new(ep);
    cli.post("/abc").send().await.assert_status_is_ok();
}

#[test]
fn deprecated() {
    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(path = "/abc", method = "get", deprecated)]
        async fn test(&self) {}
    }

    let meta: MetaApi = Api::meta().remove(0);
    assert!(meta.paths[0].operations[0].deprecated);
}

#[test]
fn tag() {
    #[derive(Tags)]
    enum MyTags {
        /// User operations
        UserOperations,
        /// Pet operations
        PetOperations,
    }

    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(
            path = "/abc",
            method = "get",
            tag = "MyTags::UserOperations",
            tag = "MyTags::PetOperations"
        )]
        async fn test(&self) {}
    }

    let meta: MetaApi = Api::meta().remove(0);
    assert_eq!(
        meta.paths[0].operations[0].tags,
        vec!["UserOperations", "PetOperations"]
    );
}

#[tokio::test]
async fn common_attributes() {
    #[derive(Tags)]
    enum MyTags {
        UserOperations,
        CommonOperations,
    }

    struct Api;

    #[OpenApi(prefix_path = "/hello", tag = "MyTags::CommonOperations")]
    impl Api {
        #[oai(path = "/world", method = "get", tag = "MyTags::UserOperations")]
        async fn test(&self) {}
    }

    let meta: MetaApi = Api::meta().remove(0);
    assert_eq!(meta.paths[0].path, "/hello/world");
    assert_eq!(
        meta.paths[0].operations[0].tags,
        vec!["CommonOperations", "UserOperations"]
    );

    let ep = OpenApiService::new(Api, "test", "1.0");
    TestClient::new(ep)
        .get("/hello/world")
        .send()
        .await
        .assert_status_is_ok();
}

#[tokio::test]
async fn request() {
    /// Test request
    #[derive(ApiRequest)]
    enum MyRequest {
        Json(Json<i32>),
        PlainText(PlainText<String>),
        Binary(Binary<Vec<u8>>),
    }

    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(path = "/", method = "get")]
        async fn test(&self, req: MyRequest) {
            match req {
                MyRequest::Json(value) => {
                    assert_eq!(value.0, 100);
                }
                MyRequest::PlainText(value) => {
                    assert_eq!(value.0, "abc");
                }
                MyRequest::Binary(value) => {
                    assert_eq!(value.0, vec![1, 2, 3]);
                }
            }
        }
    }

    let meta: MetaApi = Api::meta().remove(0);
    let meta_request = meta.paths[0].operations[0].request.as_ref().unwrap();
    assert!(meta_request.required);
    assert_eq!(meta_request.description, Some("Test request"));

    assert_eq!(meta_request.content[0].content_type, "application/json");
    assert_eq!(meta_request.content[0].schema, i32::schema_ref());

    assert_eq!(meta_request.content[1].content_type, "text/plain");
    assert_eq!(meta_request.content[1].schema, String::schema_ref());

    assert_eq!(
        meta_request.content[2].content_type,
        "application/octet-stream"
    );
    assert_eq!(
        meta_request.content[2].schema.unwrap_inline(),
        &MetaSchema {
            format: Some("binary"),
            ..MetaSchema::new("string")
        }
    );

    let ep = OpenApiService::new(Api, "test", "1.0");
    let cli = TestClient::new(ep);

    cli.get("/")
        .content_type("application/json")
        .body("100")
        .send()
        .await
        .assert_status_is_ok();

    cli.get("/")
        .content_type("text/plain")
        .body("abc")
        .send()
        .await
        .assert_status_is_ok();

    cli.get("/")
        .content_type("application/octet-stream")
        .body(vec![1, 2, 3])
        .send()
        .await
        .assert_status_is_ok();
}

#[tokio::test]
async fn payload_request() {
    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(path = "/", method = "post")]
        async fn test(&self, req: Json<i32>) {
            assert_eq!(req.0, 100);
        }
    }

    let meta: MetaApi = Api::meta().remove(0);
    let meta_request = meta.paths[0].operations[0].request.as_ref().unwrap();
    assert!(meta_request.required);

    assert_eq!(meta_request.content[0].content_type, "application/json");
    assert_eq!(meta_request.content[0].schema, i32::schema_ref());

    let ep = OpenApiService::new(Api, "test", "1.0");
    let cli = TestClient::new(ep);

    cli.post("/")
        .content_type("application/json")
        .body("100")
        .send()
        .await
        .assert_status_is_ok();

    cli.post("/")
        .content_type("text/plain")
        .body("100")
        .send()
        .await
        .assert_status(StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn response() {
    #[derive(ApiResponse)]
    enum MyResponse {
        /// Ok
        #[oai(status = 200)]
        Ok,
        /// Already exists
        #[oai(status = 409)]
        AlreadyExists(Json<u16>),
        /// Default
        Default(StatusCode, PlainText<String>),
    }

    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(path = "/", method = "get")]
        async fn test(&self, code: Query<u16>) -> MyResponse {
            match code.0 {
                200 => MyResponse::Ok,
                409 => MyResponse::AlreadyExists(Json(code.0)),
                _ => MyResponse::Default(
                    StatusCode::from_u16(code.0).unwrap(),
                    PlainText(format!("code: {}", code.0)),
                ),
            }
        }
    }

    let meta: MetaApi = Api::meta().remove(0);
    let meta_responses = &meta.paths[0].operations[0].responses;
    assert_eq!(meta_responses.responses[0].description, "Ok");
    assert_eq!(meta_responses.responses[0].status, Some(200));
    assert!(meta_responses.responses[0].content.is_empty());

    assert_eq!(meta_responses.responses[1].description, "Already exists");
    assert_eq!(meta_responses.responses[1].status, Some(409));
    assert_eq!(
        meta_responses.responses[1].content[0].content_type,
        "application/json"
    );
    assert_eq!(
        meta_responses.responses[1].content[0].schema,
        u16::schema_ref()
    );

    assert_eq!(meta_responses.responses[2].description, "Default");
    assert_eq!(meta_responses.responses[2].status, None);
    assert_eq!(
        meta_responses.responses[2].content[0].content_type,
        "text/plain"
    );
    assert_eq!(
        meta_responses.responses[2].content[0].schema,
        String::schema_ref()
    );

    let ep = OpenApiService::new(Api, "test", "1.0");
    let cli = TestClient::new(ep);

    let resp = cli.get("/").query("code", &200).send().await;
    resp.assert_status_is_ok();
    resp.assert_text("").await;

    let resp = cli.get("/").query("code", &409).send().await;
    resp.assert_status(StatusCode::CONFLICT);
    resp.assert_content_type("application/json; charset=utf-8");
    resp.assert_text("409").await;

    let resp = cli.get("/").query("code", &404).send().await;
    resp.assert_status(StatusCode::NOT_FOUND);
    resp.assert_content_type("text/plain; charset=utf-8");
    resp.assert_text("code: 404").await;
}

#[tokio::test]
async fn bad_request_handler() {
    #[derive(ApiResponse)]
    #[oai(bad_request_handler = "bad_request_handler")]
    enum MyResponse {
        /// Ok
        #[oai(status = 200)]
        Ok(PlainText<String>),
        /// Already exists
        #[oai(status = 400)]
        BadRequest(PlainText<String>),
    }

    fn bad_request_handler(err: Error) -> MyResponse {
        MyResponse::BadRequest(PlainText(format!("!!! {}", err.to_string())))
    }

    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(path = "/", method = "get")]
        async fn test(&self, code: Query<u16>) -> MyResponse {
            MyResponse::Ok(PlainText(format!("code: {}", code.0)))
        }
    }

    let ep = OpenApiService::new(Api, "test", "1.0");
    let cli = TestClient::new(ep);

    let resp = cli.get("/").query("code", &200).send().await;
    resp.assert_status_is_ok();
    resp.assert_content_type("text/plain; charset=utf-8");
    resp.assert_text("code: 200").await;

    let resp = cli.get("/").send().await;
    resp.assert_status(StatusCode::BAD_REQUEST);
    resp.assert_content_type("text/plain; charset=utf-8");
    resp.assert_text(
        r#"!!! failed to parse parameter `code`: Type "integer(uint16)" expects an input value."#,
    )
    .await;
}

#[tokio::test]
async fn bad_request_handler_for_validator() {
    #[derive(ApiResponse)]
    #[oai(bad_request_handler = "bad_request_handler")]
    enum MyResponse {
        /// Ok
        #[oai(status = 200)]
        Ok(PlainText<String>),
        /// Already exists
        #[oai(status = 400)]
        BadRequest(PlainText<String>),
    }

    fn bad_request_handler(err: Error) -> MyResponse {
        MyResponse::BadRequest(PlainText(format!("!!! {}", err.to_string())))
    }

    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(path = "/", method = "get")]
        async fn test(
            &self,
            #[oai(validator(maximum(value = "100")))] code: Query<u16>,
        ) -> MyResponse {
            MyResponse::Ok(PlainText(format!("code: {}", code.0)))
        }
    }

    let ep = OpenApiService::new(Api, "test", "1.0");
    let cli = TestClient::new(ep);

    let resp = cli.get("/").query("code", &50).send().await;
    resp.assert_status_is_ok();
    resp.assert_content_type("text/plain; charset=utf-8");
    resp.assert_text("code: 50").await;

    let resp = cli.get("/").query("code", &200).send().await;
    resp.assert_status(StatusCode::BAD_REQUEST);
    resp.assert_content_type("text/plain; charset=utf-8");
    resp.assert_text(r#"!!! failed to parse parameter `code`: verification failed. maximum(100, exclusive: false)"#).await;
}

#[tokio::test]
async fn poem_extract() {
    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(path = "/", method = "get")]
        async fn test(&self, data: Data<&i32>) {
            assert_eq!(*data.0, 100);
        }
    }

    let ep = OpenApiService::new(Api, "test", "1.0").data(100i32);
    TestClient::new(ep)
        .get("/")
        .send()
        .await
        .assert_status_is_ok();
}

#[tokio::test]
async fn returning_borrowed_value() {
    struct Api {
        value1: i32,
        value2: String,
        values: Vec<i32>,
    }

    #[OpenApi]
    impl Api {
        #[oai(path = "/value1", method = "get")]
        async fn value1(&self) -> Json<&i32> {
            Json(&self.value1)
        }

        #[oai(path = "/value2", method = "get")]
        async fn value2(&self) -> Json<&String> {
            Json(&self.value2)
        }

        #[oai(path = "/value3", method = "get")]
        async fn value3<'a>(&self, data: Data<&'a i32>) -> Json<&'a i32> {
            Json(&data)
        }

        #[oai(path = "/values", method = "get")]
        async fn values(&self) -> Json<&[i32]> {
            Json(&self.values)
        }
    }

    let ep = OpenApiService::new(
        Api {
            value1: 999,
            value2: "abc".to_string(),
            values: vec![1, 2, 3, 4, 5],
        },
        "test",
        "1.0",
    )
    .data(888i32);
    let cli = TestClient::new(ep);

    let resp = cli.get("/value1").send().await;
    resp.assert_status_is_ok();
    resp.assert_text("999").await;

    let resp = cli.get("/value2").send().await;
    resp.assert_status_is_ok();
    resp.assert_text("\"abc\"").await;

    let resp = cli.get("/value3").send().await;
    resp.assert_status_is_ok();
    resp.assert_text("888").await;

    let resp = cli.get("/values").send().await;
    resp.assert_status_is_ok();
    resp.assert_text("[1,2,3,4,5]").await;
}

#[tokio::test]
async fn external_docs() {
    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(
            path = "/",
            method = "get",
            external_docs = "https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md"
        )]
        async fn test(&self) {}
    }

    let meta: MetaApi = Api::meta().remove(0);
    assert_eq!(
        meta.paths[0].operations[0].external_docs,
        Some(MetaExternalDocument {
            url: "https://github.com/OAI/OpenAPI-Specification/blob/main/versions/3.1.0.md"
                .to_string(),
            description: None
        })
    );
}

#[tokio::test]
async fn generic() {
    trait MyApiPort: Send + Sync + 'static {
        fn test(&self) -> String;
    }

    struct MyApiA;

    impl MyApiPort for MyApiA {
        fn test(&self) -> String {
            "test".to_string()
        }
    }

    struct MyOpenApi<MyApi> {
        api: MyApi,
    }

    #[OpenApi]
    impl<MyApi: MyApiPort> MyOpenApi<MyApi> {
        #[oai(path = "/some_call", method = "get")]
        async fn some_call(&self) -> Json<String> {
            Json(self.api.test())
        }
    }

    let ep = OpenApiService::new(MyOpenApi { api: MyApiA }, "test", "1.0");
    let cli = TestClient::new(ep);

    let resp = cli.get("/some_call").send().await;
    resp.assert_status_is_ok();
    resp.assert_json("test").await;
}

#[tokio::test]
async fn extra_response_headers_on_operation() {
    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(
            path = "/",
            method = "get",
            response_header(name = "A1", type = "String", description = "abc"),
            response_header(name = "a2", type = "i32", deprecated = true)
        )]
        async fn test(&self) {}
    }

    let meta: MetaApi = Api::meta().remove(0);

    let header = &meta.paths[0].operations[0].responses.responses[0].headers[0];
    assert_eq!(header.name, "A1");
    assert_eq!(header.description.as_deref(), Some("abc"));
    assert_eq!(header.deprecated, false);
    assert_eq!(header.schema, String::schema_ref());

    let header = &meta.paths[0].operations[0].responses.responses[0].headers[1];
    assert_eq!(header.name, "A2");
    assert_eq!(header.description, None);
    assert_eq!(header.deprecated, true);
    assert_eq!(header.schema, i32::schema_ref());
}

#[tokio::test]
async fn extra_response_headers_on_api() {
    struct Api;

    #[OpenApi(
        response_header(name = "A1", type = "String", description = "abc"),
        response_header(name = "a2", type = "i32", deprecated = true)
    )]
    impl Api {
        #[oai(path = "/", method = "get")]
        async fn test(&self) {}
    }

    let meta: MetaApi = Api::meta().remove(0);

    let header = &meta.paths[0].operations[0].responses.responses[0].headers[0];
    assert_eq!(header.name, "A1");
    assert_eq!(header.description.as_deref(), Some("abc"));
    assert_eq!(header.deprecated, false);
    assert_eq!(header.schema, String::schema_ref());

    let header = &meta.paths[0].operations[0].responses.responses[0].headers[1];
    assert_eq!(header.name, "A2");
    assert_eq!(header.description, None);
    assert_eq!(header.deprecated, true);
    assert_eq!(header.schema, i32::schema_ref());
}

#[tokio::test]
async fn extra_request_headers_on_operation() {
    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(
            path = "/",
            method = "get",
            request_header(name = "A1", type = "String", description = "abc"),
            request_header(name = "a2", type = "i32", deprecated = true)
        )]
        async fn test(&self) {}
    }

    let meta: MetaApi = Api::meta().remove(0);

    let params = &meta.paths[0].operations[0].params[0];
    assert_eq!(params.name, "A1");
    assert_eq!(params.schema, String::schema_ref());
    assert_eq!(params.in_type, MetaParamIn::Header);
    assert_eq!(params.description.as_deref(), Some("abc"));
    assert_eq!(params.required, true);
    assert_eq!(params.deprecated, false);

    let params = &meta.paths[0].operations[0].params[1];
    assert_eq!(params.name, "A2");
    assert_eq!(params.schema, i32::schema_ref());
    assert_eq!(params.in_type, MetaParamIn::Header);
    assert_eq!(params.description, None);
    assert_eq!(params.required, true);
    assert_eq!(params.deprecated, true);
}

#[tokio::test]
async fn extra_request_headers_on_api() {
    struct Api;

    #[OpenApi(
        request_header(name = "A1", type = "String", description = "abc"),
        request_header(name = "a2", type = "i32", deprecated = true)
    )]
    impl Api {
        #[oai(path = "/", method = "get")]
        async fn test(&self) {}
    }

    let meta: MetaApi = Api::meta().remove(0);

    let params = &meta.paths[0].operations[0].params[0];
    assert_eq!(params.name, "A1");
    assert_eq!(params.schema, String::schema_ref());
    assert_eq!(params.in_type, MetaParamIn::Header);
    assert_eq!(params.description.as_deref(), Some("abc"));
    assert_eq!(params.required, true);
    assert_eq!(params.deprecated, false);

    let params = &meta.paths[0].operations[0].params[1];
    assert_eq!(params.name, "A2");
    assert_eq!(params.schema, i32::schema_ref());
    assert_eq!(params.in_type, MetaParamIn::Header);
    assert_eq!(params.description, None);
    assert_eq!(params.required, true);
    assert_eq!(params.deprecated, true);
}

#[tokio::test]
async fn multiple_methods() {
    struct Api;

    #[OpenApi]
    impl Api {
        #[oai(path = "/abc", method = "post", method = "put")]
        async fn test(&self) {}
    }

    let meta: MetaApi = Api::meta().remove(0);
    assert_eq!(meta.paths[0].path, "/abc");
    assert_eq!(meta.paths[0].operations[0].method, Method::POST);
    assert_eq!(meta.paths[0].operations[1].method, Method::PUT);

    let ep = OpenApiService::new(Api, "test", "1.0");
    let cli = TestClient::new(ep);

    cli.post("/abc").send().await.assert_status_is_ok();
    cli.put("/abc").send().await.assert_status_is_ok();
}
