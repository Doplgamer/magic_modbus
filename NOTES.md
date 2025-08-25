## README.MD
Title: `Magic Modbus`

Banner/Art: `ASCII art of Magic Schoolbus with hacking twist`

Objective: `CLI tool making ModBus more accessible/easy to use/hack`

### Development process
Started with: Learning how `ratatui` worked

Got a basic UI working

Realizing it's a bit complicated, I started small 
- Figured out how tabs worked
  - Put the tabs in the original .rs file
- Figured out how tables worked
- Made a dynamic table 
- Made a table with dynamic styles 
- Made the app loop non-blocking
- Combined the dynamic table and table with dynamic styles into one table
- Multiplied table by four and put it in the original app
- Began working on the bottom tabs
  - Started with networking concepts, figuring out how the app would connect
  - At first, I took a look at `pnet`, but that seemed like overkill for what I was working with
- Finally realized that I needed to make the app async
  - A few days of deep-learning later, I now know async rust
- Used `tokio`, `tokio-util`, `futures` to make a runtime, using a mpsc channel to send events from the event handler to the main runtime
- After migrating to async, realized, hey, only reason I was using `modbus` instead of `tokio-modbus` was because my app wasn't async
  - Ergo, we switch from `modbus` to `tokio-modbus`
- Realized after this, that I was storing 65k structs in memory every time I ran the program
  - Very inefficient
- Decided to go with a Hashmap to save on memory, but in order to do so, this meant refactoring a bunch of code
- Capabilities:
  - Reading a whole page, editing a single value, editing multiple values, reading multiple values (the ones just edited)
    - (using the methods to write multiple values would take too much time)
      - Ergo, stick to a Vec of single commands
- Fast-forward a week or so and an MVP is now ready. While there are still a few things to refine/finish, I decided it would be better to get more presence online