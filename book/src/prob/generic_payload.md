# Generic Message Payload
Currently the `FutureIndex` that is used on scheduling is hard-coded
with a fixed set of fields, which can make trouble when a user tries
to implement a scheduler that needs other payloads.

A fast hack is that the `FutureIndex` can be altered with an extra field
that is `Option<Box<dyn Any + Send>>`. As you can see, it is ugly.

There are a few options that can be used as an solution:
1. Serialize the extra payloads needed as a string. This is easy
as we won't alter the main structure of the runtime. However, this
will introduce extra dependencies and complexity as you need to deserialize multiple times
each time when the scheduler need to access it. The increased amount of time is depending
on the efficiency of the serialization framework and the format of the string.
2. Add an associate type to the `Scheduler` trait that can specify the payload type and create
a storage (slab or heap object pool) that contains that type. By this, the `FutureIndex` will
have a new field that contains the key to access the storage and the `Scheduler` trait will
add one more function that initiates the storage and return it. Further survey is needed
to determine the implementation detail of this and check if it's doable. There's always
a risk that at the end of the day, it is slower than the first solution with a fast serialization
framework and a format that can be serialized at lightning speed.
