import capnp
capnp.remove_import_hook()
predictor = capnp.load('../protos/predictor.capnp')

client = capnp.TwoPartyClient('127.0.0.1:12345')
pred = client.ez_restore('predictor').cast_as(predictor.Predictor)

rq = pred.predict_request()
rq.req = 2
pm = rq.send()

print(str(pm.wait()))
