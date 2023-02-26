pub mod data_lake;
use data_lake::*;



#[tokio::main]
async fn main() -> ()
{
}

#[tokio::test]
async fn multi_task_publish_subscribe()
{
    let mut datalake = TDataLake::new();
    let datalake2 = datalake.clone();


    let join1 = tokio::task::spawn(async move {
        let mut sub = datalake.subscribe::<String>(&"/test".parse().unwrap()).await;
        for i in 1..10
        {
            let data = sub.receiver.recv().await;
            println!("rx{}: {}", i, data.unwrap());
        }
    });
    let join2 = tokio::task::spawn(async move {
        for _ in 1..10
        {
            datalake2.publish::<String>(&"/test".parse().unwrap(), "hallo".into()).await;
        }
    });
    join1.await.unwrap();
    join2.await.unwrap();
    assert!(false);

}
