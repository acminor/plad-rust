import capnp
capnp.remove_import_hook()

predictor = capnp.load('../protos/predictor.capnp')

temp = predictor.Predictor.PredictRequest.new_message()

temp.lookBacks = [[i for i in range(0,2)] for j in range(0,2)]
temp.times = [i for i in range(0,2)]
temp.predictor = 'npp'

print('pred:'+str(temp.to_bytes()))

class NullPredictor(predictor.Predictor.Server):
    def predict(self, req, **kwargs):
        #print("Received req:"+req)
        print('blah')
        res = predictor.Predictor.PredictResponse.new_message()
        res.predictions = [i for i in range(0,2)]
        return res

def restore(ref):
    assert ref.as_text() == 'predictor'
    return NullPredictor()

server = capnp.TwoPartyServer('127.0.0.1:12345', restore)
server.run_forever()
