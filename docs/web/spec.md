The website shall have the following routes:

The landing page at `/`.
Create a poll at `/create`.
Vote on a poll at `/p/<poll-share-id>`.
View the results of a poll at `/p/<poll-share-id>/results`.

A URL under `/p/` with no poll behind it shall say the poll doesn't exist.
Any other unrouted URL shall say the page doesn't exist.

All pages shall have a navbar at the top of the page with the following elements on the left half:

The logo (navigates to the landing page).
An element that navigates to the create page.

The navbar shall have a light/dark/system theme toggle which is a clickable icon without text that cycles through the different modes. The default shall be system theme. See `https://pydantic.dev/docs/` for a good example of this.

The landing page should have some header text and subtitle/description, as well as prominent element that navigates to the create page. Below this should be some information on what our website is, how it works, and what features it has.

The create page shall have some header text and subtitle/description for users to create a poll. The page shall have the following fields:

Title
Description
Options

The title shall be required and limited to `200` characters. A counter or other indicator shall appear to notify the user as they approach the limit, and the input field shall not accept more characters than the limit.

The description shall be optional and limited to `2000` characters. A counter or other indicator shall appear to notify the user as they approach the limit, and the input field shall not accept more characters than the limit.

The poll options shall be initialized with `2` blank option entries, which shall be the minimum number of options required to create a poll. Option entries are required fields and are limited to `200` characters. Option entries shall have an `X` icon on their right to delete them, which shall not be visible if the number of present options is less than or equal to the minimum number of entries required to create a poll.

An element to create (append) a new option entry shall exist below the last present option entry.

Below the options shall exist an 'Additional settings' toggle which reveals the following:

Deadline

The deadline setting should be a toggle (switch) that defaults to off and contains flavor text (a short description below it that describes what the setting does). When switched on, the following shall appear:

A required datetime field.
Hide results until deadline toggle (switch), defaults to off.

Below the additional settings shall exist an element to create the poll. Upon interaction, client-side checks shall be run. If they pass, the request shall be sent to the server, and upon passing server-validation, the user shall be redirected to the voting page.

On the voting page, if the poll has a deadline that has passed, the page shall display a notice to the user that the poll closed at <deadline> and display an element that navigates to the results page. If the poll does not have a deadline that has passed, there shall exist the title of the poll, the description of the poll (if one was provided), the deadline (if one was provided), and the main voting area.

The main voting area shall have two main areas: the top area shall be the area where options are ranked, while the bottom area shall contain all options/candidates initially and be labaled as the Unranked section. This unranked section serves as the candidate bank and all options shall be lexicographically sorted to improve findability. When there are no options ranked yet, that top area shall contain some placeholder text such as 'Drag options here to rank'. Dragging-and-dropping an option in that area shall automatically create the first rank/tier. Each rank row shall have a number associated with it which corresponds with the ranking of that tier relative to the number of options above it. For example:

```
1  [Option 1]
2  [Option 3] [Option 4] [Option 5]
5  [Option 2]
```

Notice that the 3rd row/rank/tier is numbered `5` instead of `3`. This is because there are 4 options above Option 2, so Option 2 would be 5th place at best. Dragging-and-hovering an option above/below/between tiers shall provide some indication that a new tier will be created in the corresponding position upon release, and that is exactly what shall happen upon release. Dragging all options out of a tier/row shall automatically delete that tier/row as well.

Below the main voting area shall exist two elements: a submission element to submit one's vote and another element to view the results.

Below these two elements, a Share section shall exist. This shall display the poll's share URL in an element. To the right of this element shall exist a Copy to clipboard icon with no text. Upon interaction, this icon shall copy the poll's share URL to the clipboard and the icon shall change for `3` seconds to denote successful copying to the clipboard to the user.

Upon interaction with the submission element, a pop-up modal shall appear informing the user that their vote was successfully submitted and recorded, and an element shall exist in that modal that navigates to the results page on click. There shall exist an `x` in the top-right of the pop-up modal for the user to close the modal. The user may also exit the modal by clicking anywhere outside the modal.

If the poll's results are hidden until a deadline that has not passed, the results page shall display a countdown along with the deadline to inform the user when they can expect to view the results. An element should also exist that navigates to the voting page for the user to submit their vote until the deadline. When the deadline passes, the page shall automatically reveal the standings.

If the poll's results are not hidden until a deadline, the results page shall display the following:

The poll title
The poll description (if one was provided)
The number of votes submitted/cast
Live indicator (green) + countdown, OR deadline (if deadline was provided; countdown to deadline if not yet passed, otherwise deadline - Closed at <deadline>)
The winner (current winner if there exists a deadline that has not passed, otherwise it is simply the winner outright).
The standings

If the poll does not have a deadline that has passed, an element should exist below the winner that navigates to the voting page.

The standings shall be displayed as a ranked list of tiers (see `https://github.com/buffbirb/maximal-lottery-marimo/blob/07a45cd73e44cded277d747c2f9db720cafd43b3/matrix.py`). Options shall be ranked top to bottom, where each row is a ranking slot. Repeatedly extracting Condorcet winners peels off the top, and any options/candidates in a Condorcet cycle collapse into a single shared slot/row. For example:

```
1  [Option 1]

   [Option 2]    33%
2  [Option 3]    33%
   [Option 5]    33%

5  [Option 4]
```

Each option shall be clickable, which pops up a modal showing the target (selected) option's head-to-head margins against the rest of the options (ordered in the same order as the standings, omitting the target candidate itself from its own list of head-to-heads):

```
[Option 1]               x
Head-to-head margins

[Option 2]              +4
[Option 3]              -2
[Option 5]              +0
[Option 4]              +6
```

Options that the target option beats shall be colored green to denote winning. Losing matchups (negative margins) shall be red, and neutral/tied ones shall be gray.

Clicking anywhere outside of the pop-up modal shall close/dismiss it. Options within a single ranked slot / tier shall be ordered by descending probability.

Below the standings shall exist the same Share section that is on the voting page, however if a deadline exists that has passed, the Share section on the results page shall display the results URL instead.
