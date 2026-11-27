use actix_web::{get, web, HttpResponse, Responder};
use crate::db::DbPool;
use crate::soroban::SorobanClient;
use crate::scval::{decode_u64, decode_bounty_id_list};
use serde_json::json;

#[get("/api/bounties/count")]
pub async fn get_bounty_count(
    soroban: web::Data<SorobanClient>,
) -> impl Responder {
    match soroban.read_call("get_bounty_count", vec![]).await {
        Ok(result) => {
            match decode_u64(&result) {
                Ok(count) => HttpResponse::Ok().json(json!({
                    "count": count
                })),
                Err(e) => HttpResponse::InternalServerError().json(json!({
                    "error": format!("Failed to decode bounty count: {}", e)
                })),
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to call get_bounty_count: {}", e)
        })),
    }
}

#[get("/api/bounties/open")]
pub async fn get_open_bounties(
    soroban: web::Data<SorobanClient>,
) -> impl Responder {
    match soroban.read_call("get_open_bounties", vec![]).await {
        Ok(result) => {
            match decode_bounty_id_list(&result) {
                Ok(bounty_ids) => HttpResponse::Ok().json(json!({
                    "bounty_ids": bounty_ids
                })),
                Err(e) => HttpResponse::InternalServerError().json(json!({
                    "error": format!("Failed to decode open bounties: {}", e)
                })),
            }
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to call get_open_bounties: {}", e)
        })),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_bounty_count)
        .service(get_open_bounties);
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use crate::soroban::SorobanClient;
    use stellar_xdr::curr::{ScVal, ScVec, U64};

    #[actix_web::test]
    async fn test_get_bounty_count() {
        let soroban = web::Data::new(SorobanClient::mock_with_response(
            ScVal::U64(U64(42))
        ));

        let app = test::init_service(
            App::new()
                .app_data(soroban.clone())
                .service(get_bounty_count)
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/bounties/count")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["count"], 42);
    }

    #[actix_web::test]
    async fn test_get_open_bounties() {
        let bounty_ids = vec![ScVal::U64(U64(1)), ScVal::U64(U64(2)), ScVal::U64(U64(3))];
        let soroban = web::Data::new(SorobanClient::mock_with_response(
            ScVal::Vec(Some(ScVec::try_from(bounty_ids).unwrap()))
        ));

        let app = test::init_service(
            App::new()
                .app_data(soroban.clone())
                .service(get_open_bounties)
        ).await;

        let req = test::TestRequest::get()
            .uri("/api/bounties/open")
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["bounty_ids"].as_array().unwrap().len(), 3);
        assert_eq!(body["bounty_ids"][0], 1);
        assert_eq!(body["bounty_ids"][1], 2);
        assert_eq!(body["bounty_ids"][2], 3);
    }
}
