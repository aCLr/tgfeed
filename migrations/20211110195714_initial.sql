CREATE TABLE posts (
    id integer primary key autoincrement not null,
    title text,
    link text not null,
    telegram_id integer not null,
    pub_date integer not null,
    content text not null,
    chat_id integer not null
);

create table channels (
    id integer primary key autoincrement not null,
    title text not null,
    username text not null unique,
    telegram_id int not null
);