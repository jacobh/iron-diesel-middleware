extern crate iron;
extern crate diesel;
extern crate r2d2;
extern crate r2d2_diesel;

use iron::prelude::*;
use iron::{typemap, BeforeMiddleware};

use std::error::Error;
use std::sync::Arc;

/// The type of the pool stored in `DieselMiddleware`.
pub type DieselPool<T: diesel::Connection> = Arc<r2d2::Pool<r2d2_diesel::ConnectionManager<T>>>;

pub type DieselPooledConnection<T: diesel::Connection> = r2d2::PooledConnection<r2d2_diesel::ConnectionManager<T>>;

/// Iron middleware that allows for diesel connections within requests.
pub struct DieselMiddleware<T: 'static + diesel::Connection> {
  /// A pool of diesel connections that are shared between requests.
  pub pool: DieselPool<T>,
}

pub struct Value<T: 'static + diesel::Connection>(DieselPool<T>);

impl<T: diesel::Connection> typemap::Key for DieselMiddleware<T> { type Value = Value<T>; }

impl<T: diesel::Connection> DieselMiddleware<T> {

    /// Creates a new pooled connection to the given sql server. The URL is in the format:
    ///
    /// ```{none}
    /// postgresql://user[:password]@host[:port][/database][?param1=val1[[&param2=val2]...]]
    /// ```
    ///
    /// Returns `Err(err)` if there are any errors connecting to the sql database.
    pub fn new(connection_str: &str) -> Result<DieselMiddleware<T>, Box<Error>> {
        Self::new_with_config(connection_str, r2d2::Config::default())
    }
    /// Creates a new connection pool, with the ability to set your own r2d2 configuration. 
    pub fn new_with_config(
      connection_str: &str,
      config: r2d2::Config<T, r2d2_diesel::Error>
    ) -> Result<DieselMiddleware<T>, Box<Error>> {
        let manager = r2d2_diesel::ConnectionManager::<T>::new(connection_str);
        let pool = try!(r2d2::Pool::new(config, manager));

        Ok(DieselMiddleware {
          pool: Arc::new(pool),
        })
    }
}

impl<T: diesel::Connection> BeforeMiddleware for DieselMiddleware<T> {
    fn before(&self, req: &mut Request) -> IronResult<()> {
        req.extensions.insert::<DieselMiddleware<T>>(Value(self.pool.clone()));
        Ok(())
    }
}

/// Adds a method to requests to get a database connection.
///
/// ## Example
///
/// ```ignore
/// use iron_diesel_middleware::{DieselPooledConnection, DieselReqExt};
///
/// fn handler(req: &mut Request) -> IronResult<Response> {
///   let connection: DieselPooledConnection<diesel::pg::PgConnection> = req.db_conn();
///
///   let new_user = NewUser::new("John Smith", 25);
///   diesel::insert(&new_user).into(users::table).execute(&*connection);
///
///   Ok(Response::with((status::Ok, "Added User")))
/// }
/// ```
pub trait DieselReqExt<T: 'static + diesel::Connection> {
  /// Returns a pooled connection to the sql database. The connection is returned to
  /// the pool when the pooled connection is dropped.
  ///
  /// **Panics** if a `DieselMiddleware` has not been registered with Iron, or if retrieving
  /// a connection to the database times out.
  fn db_conn(&self) -> r2d2::PooledConnection<r2d2_diesel::ConnectionManager<T>>;
}

impl<'a, 'b, T: 'static + diesel::Connection> DieselReqExt<T> for Request<'a, 'b> {
  fn db_conn(&self) -> r2d2::PooledConnection<r2d2_diesel::ConnectionManager<T>> {
    let poll_value = self.extensions.get::<DieselMiddleware<T>>().unwrap();
    let &Value(ref poll) = poll_value;

    return poll.get().unwrap();
  }
}
