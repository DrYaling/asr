//#[macro_use] extern crate actix_web;
use actix_web::{post, App, HttpServer, Responder};

#[post("/index.html")]
async fn index() -> impl Responder {
    format!("Hello {}! id:{}", 1, 2)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(index))
        .bind("127.0.0.1:18080")?
        .run()
        .await
}
#[cfg(test)]
#[test]
fn type_vies(){
    #[derive(Debug)]
    struct Tv{
        pub a: i32,
        pub b: Vec<i32>,
        pub c: String,
    }
    impl Tv{
        pub fn mov(self) -> bool{
            let Tv{a,b,..} = &self;
            println!("{},{:?},{}",a,b,self.c);
            true
        }
        pub fn mb(&mut self){
            let Tv { a, b, c } = self;
            b.push(c.len() as i32);
            *a = b.len() as i32;
            c.push_str(&a.to_string());
            println!("{:?}",self)
        }
    }
    let mut tv  = Tv{a: 0, b: Default::default(), c: "init".to_string()};
    tv.mb();
    tv.mov();

    
}