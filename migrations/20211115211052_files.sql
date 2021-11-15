CREATE TABLE files (
   id integer primary key autoincrement not null,
   local_path text null,
   remote_file int not null unique ,
   remote_id text not null
);

create table post_files (
    id integer primary key autoincrement not null,
    post_id int not null,
    file_id int not null
);

