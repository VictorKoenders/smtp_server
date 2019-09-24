use postgres::{Client, NoTls, Transaction};
use smtp_server::{ConfigBuilder, Email, mailparse, async_trait, SmtpServer, tokio};
use std::fmt::Write;
use uuid::Uuid;
use fallible_iterator::FallibleIterator;
use std::net::{IpAddr, Ipv4Addr};

fn get_env(name: &str) -> String {
    match std::env::var(name) {
        Ok(v) => v,
        Err(e) => panic!("Could not find environment variable {:?}: {:?}", name, e),
    }
}

#[tokio::main]
async fn main() {
    let _ = dotenv::dotenv();
    let connection_string = get_env("DATABASE_URL");
    {
        let mut client =
            Client::connect(&connection_string, NoTls).expect("Could not connect to server");
        ensure_table_exists(&mut client);
        drop(client);
    }

    env_logger::init();
    let config = ConfigBuilder::default()
        .with_server_name("Trangar's NIH server")
        .with_hostname("localhost")
        .with_max_size(10*1024*1024)
        .with_pkcs12_certificate("identity.pfx", "").expect("Could not load identity.pfx")
        .build();

    let mut server = SmtpServer::create(Handler { connection_string }, config);
    server
        .register_listener((IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 25))
        .await
        .expect("Could not listen on port 25");
    server
        .register_listener((IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 587))
        .await
        .expect("Could not listen on port 587");
    server
        .register_tls_listener((IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 465))
        .await
        .expect("Could not listen on port 465");
    server.run().await;
}


fn insert_mail(transaction: &mut Transaction, email: &Email) -> Result<Uuid, failure::Error> {
    println!("Inserting email");
    const QUERY: &str = r#"INSERT INTO mail
    ("remote_addr", "ssl", "from")
VALUES
    ($1, $2, $3)
RETURNING id"#;
    let mut result = transaction.query_iter(
        QUERY,
        &[
            &email.peer_addr.to_string().as_str(),
            &email.used_ssl,
            &email.sender,
        ],
    )?;
    let row = result
        .next()?
        .ok_or_else(|| failure::format_err!("Query failed"))?;

    Ok(row.get(0))
}

fn insert_mail_to(transaction: &mut Transaction, id: Uuid, to: &str) -> Result<(), failure::Error> {
    println!("Inserting email_to");
    const QUERY: &str = r#"INSERT INTO mail_to
    ("mail_id", "to")
VALUES
    ($1, $2)"#;
    transaction.execute(QUERY, &[&id, &to])?;
    Ok(())
}

fn insert_mail_part(
    transaction: &mut Transaction,
    id: Uuid,
    parent_part_id: Option<Uuid>,
    part: &mailparse::ParsedMail,
) -> Result<(), failure::Error> {
    println!("Inserting mail_part");
    const QUERY: &str = r#"INSERT INTO mail_part
    ("mail_id", "parent_part_id", "body")
VALUES
    ($1, $2, $3)
RETURNING id
"#;
    let part_id: Uuid;
    {
        let mut result =
            transaction.query_iter(QUERY, &[&id, &parent_part_id, &part.get_body_raw()?])?;
        let row = result
            .next()?
            .ok_or_else(|| failure::format_err!("Could not insert mailpart"))?;
        part_id = row.get(0);
    }

    for header in &part.headers {
        println!("Inserting mail_header");
        const QUERY: &str = r#"INSERT INTO mail_header
    ("mail_part_id", "mail_id", "key", "value")
VALUES
    ($1, $2, $3, $4)
"#;
        let key = header.get_key()?;
        let value = header.get_value()?;
        transaction.execute(QUERY, &[&part_id, &id, &key.as_str(), &value.as_str()])?;
    }

    for child in &part.subparts {
        insert_mail_part(transaction, id, Some(part_id), child)?;
    }
    Ok(())
}

fn try_save_email(transaction: &mut Transaction, email: &Email) -> Result<(), failure::Error> {
    let id = insert_mail(transaction, email)?;
    insert_mail_to(transaction, id, &email.recipient)?;
    insert_mail_part(transaction, id, None, &email.email)?;
    println!("Inserted mail {:?}", id);
    Ok(())
}

struct Handler {
    connection_string: String
}

impl Handler {
    fn create_connection(&self) -> Result<Client, failure::Error> {
        let client =
            Client::connect(&self.connection_string, NoTls).expect("Could not connect to server");
        Ok(client)
    }
    fn try_save_email(&self, email: &Email) -> Result<(), String> {
        let mut conn = self.create_connection().map_err(|e| format!("Could not connect to DB server: {:?}", e))?;
        let mut transaction = conn.transaction().map_err(|e| format!("Could not create transaction: {:?}", e))?;
        match try_save_email(&mut transaction, email) {
            Ok(()) => {
                transaction.commit().map_err(|e| format!("Could not commit transaction: {:?}", e))?;
                Ok(())
            }
            Err(e) => {
                transaction.rollback().map_err(|inner_e| format!("Could not save email ({:?}) OR rollback transaction ({:?})", e, inner_e))?;
                Err(format!("Could not save email: {:?}", e))
            }
        }
        }
}

#[async_trait]
impl smtp_server::Handler for Handler {
    async fn validate_address(&self, _addr: &str) -> bool {
        true
    }

    async fn save_email<'a>(&self, email: &Email<'a>) -> Result<(), String> {
        match self.try_save_email(email) {
            Ok(()) => Ok(()),
            Err(e) => {
                eprintln!("Could not save email:");
                eprintln!("{:?}", e);
                Err(e)
            }
        }
    }

    fn clone_box(&self) -> Box<dyn smtp_server::Handler> {
        Box::new(Handler { connection_string: self.connection_string.clone() })
    }
}

fn ensure_table_exists(client: &mut Client) {
    if !table_exists(client, "mail") {
        create_table(
            client,
            "mail",
            &[
                (
                    "id",
                    "UUID NOT NULL PRIMARY KEY DEFAULT (uuid_generate_v4())",
                ),
                ("remote_addr", "TEXT NOT NULL"),
                ("ssl", "BOOLEAN NOT NULL"),
                ("from", "TEXT NOT NULL"),
                ("received_on", "TIMESTAMPTZ NOT NULL DEFAULT (NOW())"),
            ],
            &[],
        );
    }

    if !table_exists(client, "mail_to") {
        create_table(
            client,
            "mail_to",
            &[
                (
                    "id",
                    "UUID NOT NULL PRIMARY KEY DEFAULT (uuid_generate_v4())",
                ),
                ("mail_id", "UUID NOT NULL REFERENCES mail(id)"),
                ("to", "TEXT NOT NULL"),
            ],
            &[],
        );
    }

    if !table_exists(client, "mail_part") {
        create_table(
            client,
            "mail_part",
            &[
                (
                    "id",
                    "UUID NOT NULL PRIMARY KEY DEFAULT (uuid_generate_v4())",
                ),
                ("parent_part_id", "UUID NULL REFERENCES mail_part(id)"),
                ("mail_id", "UUID NOT NULL REFERENCES mail(id)"),
                ("body", "BYTEA NOT NULL"),
            ],
            &[],
        );
    }

    if !table_exists(client, "mail_header") {
        create_table(
            client,
            "mail_header",
            &[
                (
                    "id",
                    "UUID NOT NULL PRIMARY KEY DEFAULT (uuid_generate_v4())",
                ),
                ("mail_part_id", "UUID NULL REFERENCES mail_part(id)"),
                ("mail_id", "UUID NOT NULL REFERENCES mail(id)"),
                ("key", "TEXT NOT NULL"),
                ("value", "TEXT NOT NULL"),
            ],
            &[],
        );
    }
}

fn table_exists(client: &mut Client, table_name: &str) -> bool {
    const QUERY: &str = r#"SELECT EXISTS(
    SELECT 1
    FROM information_schema.tables
    WHERE table_name = $1
)"#;
    let mut result = client
        .query_iter(QUERY, &[&table_name])
        .expect("Could not execute query");
    let row = result
        .next()
        .expect("Could not get first row")
        .expect("Could not get first row");
    row.get(0)
}

/*fn drop_table(client: &mut Client, table_name: &str) {
    let query = format!("DROP TABLE \"{}\"", table_name);
    if let Err(e) = client.execute(query.as_str(), &[]) {
        eprintln!("Could not drop table {:?}", table_name);
        eprintln!("{}", query);
        eprintln!("{:?}", e);
        panic!();
    }
}*/

fn create_table(client: &mut Client, name: &str, fields: &[(&str, &str)], additional: &[&str]) {
    let mut query = String::new();
    writeln!(&mut query, "CREATE TABLE \"{}\" (", name).unwrap();
    let mut first = true;
    for (name, r#type) in fields {
        if first {
            first = false;
        } else {
            writeln!(&mut query, ",").unwrap()
        }
        write!(&mut query, "\t\"{}\" {}", name, r#type).unwrap();
    }
    for add in additional {
        if first {
            first = false;
        } else {
            writeln!(&mut query, ",").unwrap()
        }
        write!(&mut query, "\t{}", add).unwrap();
    }
    write!(&mut query, "\n)").unwrap();
    eprintln!("{}", query);
    if let Err(e) = client.execute(query.as_str(), &[]) {
        eprintln!("Could not create table");
        eprintln!("{:?}", e);
        panic!();
    }
}
