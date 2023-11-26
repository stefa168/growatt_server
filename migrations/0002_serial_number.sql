alter table inverter_messages
    add column if not exists inverter_sn varchar(64) default null;

-- We need to add to all the rows of inverter_messages of which the id corresponds to a message_data.message_id
-- the inverter_sn of the corresponding message_data row.
update inverter_messages im
set inverter_sn = (select md.value
                   from message_data md
                   where md.message_id = im.id
                     and key = 'Inverter SN')
where im.type = '"Data4"'
  and im.inverter_sn is null;