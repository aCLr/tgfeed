CREATE TABLE posts (
    id integer primary key autoincrement not null,
    title text,
    link text not null,
    guid text not null,
    pub_date integer not null,
    content text not null,
    channel_id integer not null
);

create table channels (
    id integer primary key autoincrement not null,
    title text not null,
    username text not null unique,
    link text not null unique,
    description text
);