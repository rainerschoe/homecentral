use std::any::TypeId;
use std::any::Any;
use std::collections::HashMap;

struct DataLake 
{
    subscriptions: HashMap<TypeId, Vec<Subscriber>>,
}

#[derive(Debug)]
struct Subscriber
{
    path: String,
    transmitter: Box<dyn Any>,
}

struct Fisher<T>
{
    receiver: std::sync::mpsc::Receiver<T>,
}

impl DataLake
{

    fn new() -> Self
    {
        DataLake{subscriptions: HashMap::new()}
    }

    fn publish<T : Any + Clone>(self: &mut Self, path: &str, object: T)
    {
        let type_id = object.type_id();
        let boxed_object = Box::new(object);
        let possible_subscribers = self.subscriptions.entry(type_id).or_insert(Vec::new());

        for subscriber in
        possible_subscribers.iter()
            .filter(|subscriber| {let sp = &(subscriber.path); println!("comparing {sp} and {path}"); let result = subscriber.path == path; println!("result = {}", result); result})
        {
            let sender = match subscriber.transmitter.downcast_ref::<std::sync::mpsc::Sender<T>>()
            {
                Some(boxed_sender) => boxed_sender,
                None => panic!("Publish and subscribe types do not match! This should not happen and is a programming error in the pubsub lib." )
            };
            sender.send((*boxed_object).clone()).unwrap();
        }
    }

    fn subscribe<T: 'static>(self: &mut Self, path: &str) -> Fisher<T>
    {
        let type_id = TypeId::of::<T>();

        let (tx, rx) = std::sync::mpsc::channel::<T>();

        self.subscriptions.entry(type_id).or_insert(Vec::new())
            .push(
                Subscriber{path: path.into(), transmitter : Box::new(tx)}
            );

        Fisher{receiver: rx}
    }

}

fn main() -> ()
{
}

#[test]
fn single_publish_single_subscribe()
{
    let mut datalake = DataLake::new();

    let fisher = datalake.subscribe::<&str>("test");

    datalake.publish::<&str>("test", "data");

    let asd = fisher.receiver.try_recv();
    match asd
    {
        Ok(v) => println!("RX ok: {}", v),
        Err(e) => panic!("rx failed: {}", e)
    }
}

#[test]
fn single_publish_multi_subscribe()
{
    let mut datalake = DataLake::new();

    let fisher1 = datalake.subscribe::<&str>("test");
    let fisher2 = datalake.subscribe::<&str>("test");

    datalake.publish::<&str>("test", "data");

    let asd = fisher1.receiver.try_recv();
    match asd
    {
        Ok(v) => println!("RX ok: {}", v),
        Err(e) => panic!("rx failed: {}", e)
    }

    let asd = fisher2.receiver.try_recv();
    match asd
    {
        Ok(v) => println!("RX ok: {}", v),
        Err(e) => panic!("rx failed: {}", e)
    }
}