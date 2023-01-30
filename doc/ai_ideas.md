# Ideas for Umpire AI

Currently using a naive Deep Q-learning Network.

Possible improvements / alternatives:

* Double
* Dueling
* Prioritized replay memory
* Generate state,action,reward tuples and train model the reward directly as a supervised learning problem
* Monte Carlo Tree Search:
 - Could use this offline to annotate the s,a,r tuple with an r' that is the sum of rewards upon fully-random rollout over a finite number of iterations or until game termination. With enough such rollouts we could estimate the long-term reward directly instead of using the Bellman equations. This would require optimizing the game engine for throughput.
 - Could use this online to give a degree of planning to the DQN.
* Continuing the datagen approach, we could get rid of all the intermediate rewards and use the AlphaGo approach with eventual victory being 1 and eventual defeat being 0. We then model victory/defeat probability and choose the action which maximizes expected victory.

A key issue we're facing is the composite nature of player turns. A player can take not one but a great number of actions in a single turn. This separates actions from longer-term rewards, leading to mass behaviors but not individual ones.

