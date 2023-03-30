pub mod data_lake;
use data_lake::*;
//use tokio_stream::stream_ext::StreamExt;
//use bus::MessagePackBusAccess;
//use bus::ConnectionSetup;

use regex::Regex;

pub mod bus {
    tonic::include_proto!("_");
}
//use futures::stream::Stream;
use tokio_stream::StreamExt;


// TODO: could use drop trait instead?
pub trait NodeHandle
{
    fn stop(self: Self);
}

mod BusAccess
{
    use crate::*;
    pub struct BusAccessHandle
    {
        stop_sender: tokio::sync::oneshot::Sender<()>,
        join_handle: tokio::task::JoinHandle::<()>,
    }

    impl crate::NodeHandle for BusAccessHandle
    {
        fn stop(self: Self)
        {
            self.stop_sender.send(());
            tokio::runtime::Handle::try_current().unwrap().block_on(self.join_handle);
        }
    }

    pub fn create(datalake: TDataLake, server_url: String, datalake_base_path: String) -> BusAccessHandle
    {
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let join_handle = tokio::task::spawn(
            receive_from_bus_and_publish(datalake, server_url, datalake_base_path, rx)
        );

        BusAccessHandle{stop_sender: tx, join_handle: join_handle}
    }
    async fn receive_from_bus_and_publish(datalake: TDataLake, server_url: String, publish_base_path: String, mut stop_receiver: tokio::sync::oneshot::Receiver<()>)
    {
        let mut client = bus::message_pack_bus_access_client::MessagePackBusAccessClient::connect(server_url).await.unwrap();

        //let request = tonic::Request::new(bus::ConnectionSetup {
        //    rx_mask: "255:255".into(),
        //    remote_address: "255:255".into(),
        //});
        let req = crate::bus::ConnectionSetup {
            rx_mask: "0:255".into(),
            remote_address: "0:1".into(),
        };
        //let req = bus::SendJsonMessageRequest {
        //    remote_address: "200:1".into(),
        //    data: "{}".into(),
        //    timeout_milliseconds: 5000
        //};
        let response = client.receive(req).await.unwrap();
        // I think proto filename needs to match

        let mut resp_stream = response.into_inner();
        
        loop
        {
            //let wait = futures::future::select(resp_stream.next(), stop_receiver);
            //match wait.await 
            tokio::select!
            {
                grpc_event = resp_stream.next() =>
                //Either::Left((grpc_event, _)) => 
                {
                    if let Some(received) = grpc_event
                    {
                        let received = received.unwrap();
                        println!("\treceived message: `{:?}`", received);

                        let re = Regex::new(r".*([0-9]+):.*").unwrap();
                        let captures = re.captures(received.remote_address.as_str());
                        if let Some(captures) = captures 
                        {
                            if let Some(deviceId) = captures.get(1)
                            {
                                datalake.publish(&("/bus/rx/".to_owned() + deviceId.as_str()).as_str().parse().unwrap(), received.data).await;
                            }
                        }
                    }
                    else 
                    {
                        // TODO reconnect here
                        break;
                    }
                }
                _ = &mut stop_receiver =>
                //Either::Right(_) =>
                {
                    // Quit
                    break;
                }
            }
        }
    }
}

//impl Drop for BusAccess 
//{
//}

#[tokio::main]
async fn main() -> ()
{
    let mut datalake = TDataLake::new();

    let datalake_bus = datalake.clone();
    let bus_handle = BusAccess::create(datalake.clone(), "http://192.168.0.200:50051".into(), "bus/receive/ug".into());


    let mut sub = datalake.subscribe::<String>(&"/bus/rx/*".parse().unwrap()).await;
    loop
    {
        let data = sub.receiver.recv().await;
        println!("rx: {}", data.unwrap());
    }

    bus_handle.stop();

    //join1.await.unwrap();
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
