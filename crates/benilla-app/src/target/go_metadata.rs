//! Do not turn missing interaction metadata into a lockless use packet.
use super::lock::GoLockInputs;
use crate::net::NetCommands;

pub(super) fn ready(inputs: &GoLockInputs, guid: u64, net: &NetCommands) -> bool {
    let Some(fields) = inputs.objects.object(guid) else {
        return false;
    };
    let Some(template) = inputs.templates.of(fields) else {
        inputs.templates.request_at_create(guid, fields, net);
        return false;
    };
    // A confirmed lockless object does not need Lock.dbc. An absent declared row
    // is incomplete local data, not proof that the object is unlocked.
    template.lock_id == 0
        || inputs
            .locks
            .as_deref()
            .is_some_and(|locks| locks.0.slots(template.lock_id).is_some())
}
