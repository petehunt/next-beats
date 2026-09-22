/// A tiny smoke test for every Rust integration surface.
pub fn hello(name: &str) -> String {
    format!("Hello, {name}, from Rust!")
}

pub const SESSION_COOKIE: &str = "beats-user";

use serde::Deserialize;
use sqlx::{PgPool, postgres::PgPoolOptions};

#[derive(Debug, PartialEq, Eq)]
pub enum ProxyDecision {
    Continue,
    Redirect { pathname: &'static str },
}

/// Evaluate the auth gate shared by the Next proxy and the Rust front door.
pub fn proxy_decision(pathname: &str, cookie_header: Option<&str>) -> ProxyDecision {
    let is_authed = cookie_header.is_some_and(|header| has_cookie(header, SESSION_COOKIE));

    if pathname != "/login" && !is_authed {
        ProxyDecision::Redirect { pathname: "/login" }
    } else {
        ProxyDecision::Continue
    }
}

fn has_cookie(header: &str, name: &str) -> bool {
    header.split(';').any(|cookie| {
        cookie
            .trim()
            .split_once('=')
            .is_some_and(|(cookie_name, value)| cookie_name == name && !value.is_empty())
    })
}

fn cookie_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').find_map(|cookie| {
        let (cookie_name, value) = cookie.trim().split_once('=')?;
        (cookie_name == name && !value.is_empty()).then_some(value)
    })
}

#[derive(Debug, PartialEq, Eq)]
pub struct PlayOutcome {
    pub status: u16,
    pub revalidation_tags: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PlayError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("track not found")]
    TrackNotFound,
}

#[derive(Clone)]
pub struct App {
    pool: PgPool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayBody {
    track_id: String,
}

impl App {
    pub fn connect_lazy(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_lazy(database_url)?;
        Ok(Self { pool })
    }

    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Handle the complete play mutation, including its shared cache-tag
    /// invalidation. The caller only adapts the outcome to its runtime response.
    pub async fn handle_play(
        &self,
        cookie_header: Option<&str>,
        body: &[u8],
    ) -> Result<PlayOutcome, PlayError> {
        let Some(user_id) = cookie_header.and_then(|header| cookie_value(header, SESSION_COOKIE))
        else {
            return Ok(PlayOutcome {
                status: 401,
                revalidation_tags: vec![],
            });
        };

        let user_exists: bool =
            sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM "User" WHERE "id" = $1)"#)
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;
        if !user_exists {
            return Ok(PlayOutcome {
                status: 401,
                revalidation_tags: vec![],
            });
        }

        let Ok(body) = serde_json::from_slice::<PlayBody>(body) else {
            return Ok(PlayOutcome {
                status: 400,
                revalidation_tags: vec![],
            });
        };
        if body.track_id.is_empty() {
            return Ok(PlayOutcome {
                status: 400,
                revalidation_tags: vec![],
            });
        }

        let mut transaction = self.pool.begin().await?;
        let updated =
            sqlx::query(r#"UPDATE "Track" SET "playCount" = "playCount" + 1 WHERE "id" = $1"#)
                .bind(&body.track_id)
                .execute(&mut *transaction)
                .await?;
        if updated.rows_affected() == 0 {
            transaction.rollback().await?;
            return Err(PlayError::TrackNotFound);
        }

        sqlx::query(
            r#"
            INSERT INTO "UserTrackPlay" ("userId", "trackId", "lastPlayedAt")
            VALUES ($1, $2, CURRENT_TIMESTAMP)
            ON CONFLICT ("userId", "trackId")
            DO UPDATE SET "lastPlayedAt" = CURRENT_TIMESTAMP
            "#,
        )
        .bind(user_id)
        .bind(&body.track_id)
        .execute(&mut *transaction)
        .await?;

        let revalidation_tags = play_revalidation_tags(user_id);
        for tag in &revalidation_tags {
            sqlx::query(
                r#"
                INSERT INTO "RustCacheTag" ("tag", "revalidatedAt", "expireSeconds")
                VALUES (
                  $1,
                  (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::bigint,
                  $2
                )
                ON CONFLICT ("tag") DO UPDATE SET
                  "revalidatedAt" = GREATEST(
                    "RustCacheTag"."revalidatedAt",
                    EXCLUDED."revalidatedAt"
                  ),
                  "expireSeconds" = CASE
                    WHEN EXCLUDED."revalidatedAt" >= "RustCacheTag"."revalidatedAt"
                    THEN EXCLUDED."expireSeconds"
                    ELSE "RustCacheTag"."expireSeconds"
                  END
                "#,
            )
            .bind(tag)
            .bind(31_536_000_i32)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;

        Ok(PlayOutcome {
            status: 204,
            revalidation_tags,
        })
    }
}

pub fn play_revalidation_tags(user_id: &str) -> Vec<String> {
    ["recently-played", "discover", "recommendations"]
        .map(|prefix| format!("{prefix}:{user_id}"))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_hello() {
        assert_eq!(hello("Next.js"), "Hello, Next.js, from Rust!");
    }

    #[test]
    fn redirects_anonymous_page_requests() {
        assert_eq!(
            proxy_decision("/library", None),
            ProxyDecision::Redirect { pathname: "/login" }
        );
    }

    #[test]
    fn permits_login_and_authenticated_requests() {
        assert_eq!(proxy_decision("/login", None), ProxyDecision::Continue);
        assert_eq!(
            proxy_decision("/library", Some("theme=dark; beats-user=user-1")),
            ProxyDecision::Continue
        );
    }

    #[test]
    fn does_not_accept_cookie_name_prefixes_or_empty_values() {
        assert!(matches!(
            proxy_decision("/", Some("not-beats-user=user-1; beats-user=")),
            ProxyDecision::Redirect { .. }
        ));
    }

    #[test]
    fn derives_play_cache_tags() {
        assert_eq!(
            play_revalidation_tags("user-1"),
            [
                "recently-played:user-1",
                "discover:user-1",
                "recommendations:user-1",
            ]
        );
    }
}
