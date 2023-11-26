-- First the table that contains the raw messages. Useful for future retrofit.
create table if not exists inverter_messages
(
    id     serial
        constraint inverter_messages_pk
            primary key,
    raw    bytea not null,
    type   text  not null,
    header bytea not null,
    time   timestamp with time zone
);

-- Now the table that holds all the extracted data from the raw messages.
create table if not exists message_data
(
    message_id integer not null
        constraint message_data_inverter_messages_id_fk
            references inverter_messages,
    key        text    not null,
    value      text,
    constraint message_data_pk
        primary key (message_id, key)
);

create index if not exists message_data_key_index
    on message_data (key);

create index if not exists message_data_message_id_index
    on message_data (message_id);

