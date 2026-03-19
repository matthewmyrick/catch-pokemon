# Battling

I want to have battling where you can open a terminal and type battle then you wait in line to find someone to battle

## Pokemon ranking and strength index

Each pokemon will have a power rank where this index will be thrown into a formula to help currate the odds of the battle

## Typing

Depending on the battle typing advantage this will give u a slight edge on teams when we run the formula

## Battle Format

When u press battle it will put your connection iunto the server.. you can only have one at a time
it waits for another user

once you have found another user to battle it will show a view of the other players PC 

then both have 15 min to select what pokemon to select (6 pokemon). So essentially i can see what pokemon u have and i could try and predict which ones u may pick and try and select accordingly. 

Then we account for typing, evolution, strength index, etc for your teams odds against the other player. Then based on those odds we run a random num or something to see who will win! Then we run it back. It is a best 3/5 game each round we pick new pokemon. It must also show what pokemon that player picked! 

## Architecture

This section needst o be a discussion because i am usnreu how this game server should work

### API

there needs to be an api to join the game right? Also to authenticate the user with oauth. 

### Battle server

Once the user is authenticated they can sit this this room watiting for others to battle them!

### Database

We will have a database of the users and their rankings, how many wins they got, and anything else that would cool to track!

### Authenitication

We will be using oauth with gmail where you login with gmail and ill set up the oauth

## local PC data

everytime you put PC we need to show the power rank somewhere as well
