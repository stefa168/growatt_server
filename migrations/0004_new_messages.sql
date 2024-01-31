-- Add migration script here
create index if not exists inverter_messages_inverter_sn_index
    on inverter_messages (inverter_sn);

create index if not exists inverter_messages_time_index
    on inverter_messages (time);

create index if not exists inverter_messages_type_index
    on inverter_messages (type);

UPDATE inverter_messages
SET type = replace(type, '"', '');