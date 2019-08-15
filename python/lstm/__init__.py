from .predictor import LSTM_Predictor as LSTMP
import logging as log
import numpy as np

class PredictorWrapper:
    def __init__(self, lstm):
        self._predictor = lstm
    def predict(self, look_backs, _times):
        look_backs = np.array(look_backs)
        res = list(self._predictor.predict(look_backs))
        return res

def build_predictor(**kwargs):
    log.debug('LSTM_Predictor build_predictor args: {}'.format(kwargs))
    lpred = LSTMP(int(kwargs['look_back']))
    lpred.load_model_weights(kwargs['arima_model_file'])
    return PredictorWrapper(lpred)
