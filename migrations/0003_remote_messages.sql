create table if not exists remote_messages
(
    id   serial
        constraint remote_messages_pk
            primary key,
    raw  bytea                    not null,
    time timestamp with time zone not null
);

alter table if exists remote_messages
    owner to postgres;