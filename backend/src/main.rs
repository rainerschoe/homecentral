use std::any::TypeId;
use std::any::Any;
use std::collections::HashMap;

pub mod PathTree;

struct DataLake 
{
    subscriptions: HashMap<TypeId, PathTree::PathTree<Subscriber>>,
}

#[derive(Debug)]
struct Subscriber
{
    path: String,
    transmitter: Box<dyn Any>,
}

struct Fisher<T>
{
    receiver: tokio::sync::mpsc::Receiver<T>,
}

impl DataLake
{

    fn new() -> Self
    {
        DataLake{subscriptions: HashMap::new()}
    }

    async fn publish<T : 'static /* for TypeId */ + Clone /* for sending to multi subscribers */ + std::fmt::Debug /* for tokio mpsc */>(self: &mut Self, path: &str, object: T)
    {
        let type_id = TypeId::of::<T>();
        let boxed_object = Box::new(object);
        let possible_subscribers = self.subscriptions.entry(type_id).or_insert(PathTree::PathTree::<Subscriber>::new()); // FIXME: should not insert here!

        // TODO: migrate following code to tree structure
        // for subscriber in
        // possible_subscribers.iter()
        //     .filter(|subscriber| {let sp = &(subscriber.path); println!("comparing {sp} and {path}"); let result = subscriber.path == path; println!("result = {}", result); result})
        // {
        //     let sender = match subscriber.transmitter.downcast_ref::<tokio::sync::mpsc::Sender<T>>()
        //     {
        //         Some(boxed_sender) => boxed_sender,
        //         None => panic!("Publish and subscribe types do not match! This should not happen and is a programming error in the pubsub lib." )
        //     };
        //     sender.send((*boxed_object).clone()).await.unwrap(); // FIXME: handle error here (receiver dropped)
        // }
    }

    fn subscribe<T: 'static>(self: &mut Self, path: &str) -> Fisher<T>
    {
        let type_id = TypeId::of::<T>();

        let (tx, rx) = tokio::sync::mpsc::channel::<T>(10); // Buffer of hard coded size for now, if more elements queued, backpressure active i.e. send() will block

        // TODO: adapt following code to tree:
        // self.subscriptions.entry(type_id).or_insert(Vec::new())
        //     .push(
        //         Subscriber{path: path.into(), transmitter : Box::new(tx)}
        //     );

        Fisher{receiver: rx}
    }
}

#[tokio::main]
async fn main() -> ()
{
}

// #[tokio::test]
// async fn single_publish_single_subscribe()
// {
//     let mut datalake = DataLake::new();
// 
//     let mut fisher = datalake.subscribe::<&str>("test");
// 
//     datalake.publish::<&str>("test", "data").await;
// 
//     let asd = fisher.receiver.try_recv();
//     match asd
//     {
//         Ok(v) => println!("RX ok: {}", v),
//         Err(e) => panic!("rx failed: {}", e)
//     }
// }
// 
// #[tokio::test]
// async fn single_publish_multi_subscribe()
// {
//     let mut datalake = DataLake::new();
// 
//     let mut fisher1 = datalake.subscribe::<&str>("test");
//     let mut fisher2 = datalake.subscribe::<&str>("test");
// 
//     datalake.publish("test", "data").await;
// 
//     let asd = fisher1.receiver.try_recv();
//     match asd
//     {
//         Ok(v) => println!("RX ok: {}", v),
//         Err(e) => panic!("rx failed: {}", e)
//     }
// 
//     let asd = fisher2.receiver.try_recv();
//     match asd
//     {
//         Ok(v) => println!("RX ok: {}", v),
//         Err(e) => panic!("rx failed: {}", e)
//     }
// }