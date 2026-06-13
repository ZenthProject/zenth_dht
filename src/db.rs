use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use std::sync::OnceLock;

pub type DbPool = Pool<ConnectionManager<PgConnection>>;
pub type DbConn = PooledConnection<ConnectionManager<PgConnection>>;

static POOL: OnceLock<DbPool> = OnceLock::new();

pub fn init_pool(database_url: &str) -> DbPool {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        .max_size(10)
        .connection_timeout(std::time::Duration::from_secs(5))
        .build(manager)
        .expect("Failed to create DB connection pool")
}

pub fn init_global_pool(pool: DbPool) {
    POOL.set(pool).ok();
}

pub fn establish_connection() -> DbConn {
    POOL.get()
        .expect("Pool not initialized")
        .get()
        .expect("Failed to get DB connection from pool")
}

pub fn try_establish_connection() -> Result<DbConn, diesel::r2d2::PoolError> {
    POOL.get()
        .expect("Pool not initialized")
        .get()
}
