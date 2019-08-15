# Predictor RPC interface
# -- might add additional interfaces for persistent GPU memory TODO

@0xedfba81d6cf6c6ba;

interface Predictor {
  init @0 (kwargs :Map(Text,Text)) -> (uid :UInt32);
  predict @1 (req :Int32) -> (res :PredictResponse);

  struct PredictRequest {
     lookBacks @0 :List(List(Float32));
     times @1 :List(Float32);
     predictor @2 :Text;
  }

  struct PredictResponse {
    predictions @0 :List(Float32);
  }

  struct Map(Key, Value) {
     entries @0 :List(Entry);
     struct Entry {
        key @0 :Key;
        val @1 :Value;
     }
  }
}