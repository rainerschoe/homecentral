use std::any::TypeId;
use std::any::Any;
use std::collections::HashMap;

pub mod path_tree;
    use path_tree::*;

struct DataLake 
{
    subscriptions: HashMap<TypeId, path_tree::PathTree<Subscriber>>,
}

#[derive(Debug)]
struct Subscriber
{
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

    async fn publish
    <
    T : 'static /* for TypeId */ + Clone /* for sending to multi subscribers */ + std::fmt::Debug /* for tokio mpsc */
    >
    (self: &mut Self, path: &path_tree::Path, object: T)
    {
        let type_id = TypeId::of::<T>();
        let boxed_object = Box::new(object);
        let possible_subscribers_opt = self.subscriptions.get(&type_id);

        match possible_subscribers_opt
        {
            Some(possible_subscribers) =>
            {
                for subscriber in
                possible_subscribers.get_payloads(path)
                {
                    let sender = match subscriber.transmitter.downcast_ref::<tokio::sync::mpsc::Sender<T>>()
                    {
                        Some(boxed_sender) => boxed_sender,
                        None => panic!("Publish and subscribe types do not match! This should not happen and is a programming error in the pubsub lib." )
                    };
                    sender.send((*boxed_object).clone()).await.unwrap(); // FIXME: handle error here (receiver dropped)
                }
            }
            None => return
        }
    }

    fn subscribe_simple<T: 'static, P: AsRef<str>>(self: &mut Self, path: P) -> Fisher<T>
    {
        self.subscribe(&path.as_ref().parse().unwrap())
    }

    fn subscribe<T: 'static>(self: &mut Self, path: &Path) -> Fisher<T>
    {
        let type_id = TypeId::of::<T>();

        let (tx, rx) = tokio::sync::mpsc::channel::<T>(10); // Buffer of hard coded size for now, if more elements queued, backpressure active i.e. send() will block
        self.subscriptions
            .entry(type_id)
            .or_insert(path_tree::PathTree::<Subscriber>::new())
            .add_payload(
                path, 
                Subscriber{transmitter : Box::new(tx)}
             );

        Fisher{receiver: rx}
    }
}

#[tokio::main]
async fn main() -> ()
{
}

#[tokio::test]
async fn single_publish_single_subscribe()
{
    let mut datalake = DataLake::new();

    let mut fisher = datalake.subscribe::<&str>(&"/test".parse().unwrap());

    datalake.publish::<&str>(&"/test".parse().unwrap(), "data").await;

    let asd = fisher.receiver.try_recv();
    match asd
    {
        Ok(v) => println!("RX ok: {}", v),
        Err(e) => panic!("rx failed: {}", e)
    }
}

#[tokio::test]
async fn single_publish_multi_subscribe()
{
    let mut datalake = DataLake::new();

    let test_path = "/test".parse::<path_tree::Path>().unwrap();
    let mut fisher1 = datalake.subscribe::<&str>(&test_path);
    //let mut fisher2 = datalake.subscribe::<&str>(test_path);
    let mut fisher2 = datalake.subscribe_simple::<&str,_>("/test");

    datalake.publish(&test_path, "data").await;

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