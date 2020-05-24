import os

import tensorflow as tf
from tensorflow.keras import Input, Model
from tensorflow.keras.layers import Dense
from tensorflow.keras.losses import MeanSquaredError

class Wrapper(tf.Module):
    def __init__(self, model):
        self.model = model

    # @tf.function()
    # def fit1(self, x, true_value):
    #     self.model.fit(x, { 'y_pred1': true_value })

    # @tf.function()
    # def fit2(self, x, true_value):
    #     self.model.fit(x, { 'y_pred2': true_value })

    @tf.function()
    def fit(self, x, y):
        self.model.fit(x, y)

if __name__=="__main__":
    os.environ['CUDA_VISIBLE_DEVICES'] = ""
    
    x = Input(shape=(1,), name='x')

    y = Input(shape=(1,), name='y')

    # y_pred1 = Dense(1, name='y_pred1')(x)
    # y_pred2 = Dense(1, name='y_pred2')(x)
    y_hat = Dense(1, name='y_hat')(x)

    mse = tf.keras.losses.MeanSquaredError(name='mse')
    loss = mse(y_hat, y)
    
    model = Model(inputs=[x, y], outputs=[loss])

    # model = Model(inputs=[x], outputs=[y_hat])

    model.compile(
        optimizer="sgd",
        loss="mse"
    )

    model.save('ai/simple_graph', save_format='tf', include_optimizer=True)

    mse = tf.keras.losses.MeanSquaredError()
    
    # y = tf.Variable(tf.constant([[1]]))
    
    # loss = mse(y, y_hat)

    model.optimizer.get_gradients(loss, model.trainable_variables + [y])



    # wrapper = Wrapper(model)

    # signature = wrapper.fit.get_concrete_function(
    #     x = tf.TensorSpec(shape=[None, 1], dtype=tf.float32, name='x'),
    #     y = tf.TensorSpec(shape=[None, 2], dtype=tf.float32, name='y'),
    # )

    # tf.saved_model.save(wrapper, "ai/blah", signatures=[signature]

    # print(model(tf.constant([[5.0]])))

    # mse = MeanSquaredError(reduction=tf.keras.losses.Reduction.NONE)

    



    # for x in range(10):
    #     x = tf.constant([[float(x)]])
    #     model.fit(x, {
    #         'y_pred1': 2.0 * x,
    #         # 'y_pred2': tf.constant([30.0]),
    #     })

    #     model.fit(x, {
    #         # 'y_pred1': tf.constant([40.0]),
    #         'y_pred2': 3.0 * x,
    #     })

    #     print(model.get_weights())

    # # def loss():
    # #     y_pred = model(x)
    # #     print(y_pred)
    # #     mse(y_pred, y)

    # # print(loss())
    
    # # optimizer = tf.keras.optimizers.Adam()
    # # train_op = optimizer.minimize(loss, model.trainable_variables, name="train")

