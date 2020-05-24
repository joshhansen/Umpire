import tensorflow as tf
from tensorflow.keras import Input, Model
from tensorflow.keras.layers import Dense

fX = tf.float32

class TrainableAI(tf.Module):
    def __init__(self, keras_model, *args, **kwargs):
        self.model = keras_model
        super().__init__(*args, **kwargs)

    @tf.function(input_signature=[
        tf.TensorSpec(shape=(1,14,), dtype=fX),#1d_features
        tf.TensorSpec(shape=(1,19,), dtype=fX),# true_action_values
    ])
    def fit_action(self, _1d_features, true_action_values):
        estimated_action_values = self.model(_1d_features)
        mse = tf.keras.losses.MeanSquaredError()
        loss = mse(estimated_action_values, true_action_value)


input = Input(shape=(14,), name='1d_features', dtype=fX)
dense = Dense(64, activation='relu', name="dense0")(input)
action_value_estimate = Dense(1, activation='linear', name='estimate')(dense)
true_action_value = Input([1], dtype=fX, name='true_action_values')

model = Model(inputs=input, outputs=action_value_estimate, name='umpire_regressor')
trainable_ai = TrainableAI(model)

tf.saved_model.save(trainable_ai, 'ai/umpire_regressor')