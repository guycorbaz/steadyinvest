use backend::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn can_get_tickers() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/api/tickers/").await;
        assert_eq!(res.status_code(), 200);

        // you can assert content like this:
        // assert_eq!(res.text(), "content");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_get_search() {
    request::<App, _, _>(|request, _ctx| async move {
        let res = request.get("/tickers/search").await;
        assert_eq!(res.status_code(), 200);
    })
    .await;
}

