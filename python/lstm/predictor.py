# adapted from and following https://machinelearningmastery.com/time-series-prediction-lstm-recurrent-neural-networks-python-keras/

import numpy as np
import matplotlib.pyplot as plt
import math
from keras.models import Sequential
from keras.layers import Dense, LSTM
from sklearn.preprocessing import MinMaxScaler
from sklearn.metrics import mean_squared_error

class LSTM_Predictor:
    def __init__(self, look_back=1):
        # for reproducibility
        np.random.seed(7)

        self.model = self._create_model(look_back)
        self._look_back = look_back


    def _norm_dataset(self):
        self.scaler = MinMaxScaler(feature_range=(0,1))
        self._dataset = scaler.fit_transform(self._dataset)


    @staticmethod
    def _create_dataset(dataset, look_back):
        data_x, data_y = [], []
        for i in range(0, len(dataset)-1 - look_back):
            a = dataset[i:(i+look_back)]
            data_x.append(a)
            data_y.append(dataset[i+look_back])
        return np.array(data_x), np.array(data_y)


    @staticmethod
    def _create_model(look_back):
        model = Sequential()
        model.add(LSTM(4, input_shape=(1, look_back)))
        model.add(Dense(1))
        model.compile(loss='mean_squared_error', optimizer='adam')
        return model


    def save_model_weights(self, loc):
        self.model.save_weights(loc)


    def load_model_weights(self, loc):
        self.model.load_weights(loc)


    def predict(self, x):
        x = np.reshape(x, (x.shape[0], 1, x.shape[1]))
        out = np.reshape(self.model.predict(x), (-1))
        return out


    def fit(self, dataset, train_split=.67):
        train_size = int(len(dataset)*train_split)
        test_size = len(dataset) - train_size
        train, test = dataset[0:train_size],\
            dataset[train_size:]

        train_x, train_y = self._create_dataset(train, self._look_back)
        test_x, test_y = self._create_dataset(test, self._look_back)

        train_x = np.reshape(train_x,
                             (train_x.shape[0], 1, train_x.shape[1]))
        test_x = np.reshape(test_x,
                            (test_x.shape[0], 1, test_x.shape[1]))

        self.model.fit(train_x, train_y, epochs=2, batch_size=1, verbose=2)
