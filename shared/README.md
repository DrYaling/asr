  
# shared
  shared library for asr framework

# structures
  * aes
      aes de/encrypt test code
  * db
      mysql pool using sqlx
  * libconfig
      json config types using serde_json
  * map
      a 2-d square map defination
  * proto
      net proto using protobuf-3.2
  * server
      network framework running with tokio,with **sync** mode and **async** mode channel and context
      * channel 
        tcp service from server to server
      * context 
        server side logic trait used for receiving client messages or sleep(delay) events (for example heart beat)
  * other
      some usefull tools 
      * net_core 
        net bytes de/encode logic
      * boxed 
        mutex faster than std mutex, especially in one thead