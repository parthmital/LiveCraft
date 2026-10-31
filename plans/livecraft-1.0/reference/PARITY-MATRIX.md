# Parity Matrix

Purpose: every Live 12.4 manual section and device, the milestone that delivers it, and its status. Read when: expanding a milestone, finishing a feature, or reporting progress.
Last verified: 2026-10-31.

- Rows are generated from the manual's table of contents (level 2 sections; level 3 for chapter 33 devices). Pages are manual page numbers.
- Status values: Missing, Prototype (some code exists, not at parity), In Progress, Done (meets `QUALITY-GATES.md#definition-of-done-every-live-feature`, with evidence), Out of Scope (see `DECISIONS.md`).
- Update the Status and Evidence cells in the same commit as the work. Evidence is a test name, a capture comparison report path, or a milestone review entry.
- Out of scope by decision D04 and D05: Max for Live itself (32.1 to 32.6 are replaced by M42 scripting), Splice and account features, Push firmware.

| Section | Title                                                         | Pages     | Milestone | Status    | Evidence |
| ------- | ------------------------------------------------------------- | --------- | --------- | --------- | -------- |
| 2.1     | Installation and Authorization                                | 24        | M45       | Missing   |          |
| 2.2     | Learning About Live                                           | 24-30     | M40       | Missing   |          |
| 2.3     | Live's Settings                                               | 31-34     | M40       | Missing   |          |
| 3.1     | The Control Bar                                               | 35        | M14       | Prototype |          |
| 3.2     | The Status Bar                                                | 36        | M14       | Missing   |          |
| 3.3     | The Browser                                                   | 37        | M24       | Prototype |          |
| 3.4     | Sound Similarity                                              | 38-39     | M24       | Missing   |          |
| 3.5     | Live Sets                                                     | 40        | M09       | Prototype |          |
| 3.6     | Arrangement and Session                                       | 40-41     | M15       | Prototype |          |
| 3.7     | Tracks                                                        | 42-44     | M03       | In Progress | milestones/M03-document-model-v2.md Review |
| 3.8     | Audio and MIDI                                                | 45        | M03       | In Progress | milestones/M03-document-model-v2.md Review |
| 3.9     | Audio Clips and Samples                                       | 46        | M11       | Missing   |          |
| 3.10    | MIDI Clips and MIDI Files                                     | 47        | M10       | Missing   |          |
| 3.11    | Devices                                                       | 48-49     | M25       | Missing   |          |
| 3.12    | Clip and Device View                                          | 50        | M14       | Missing   |          |
| 3.13    | Scale Awareness                                               | 51-52     | M35       | Missing   |          |
| 3.14    | The Mixer                                                     | 53-55     | M20       | Prototype |          |
| 3.15    | Presets and Racks                                             | 56        | M25       | Missing   |          |
| 3.16    | Routing                                                       | 56        | M20       | Missing   |          |
| 3.17    | Recording New Clips                                           | 57-58     | M21       | Missing   |          |
| 3.18    | Automation Envelopes                                          | 59        | M22       | Missing   |          |
| 3.19    | Clip Envelopes                                                | 59        | M22       | Missing   |          |
| 3.20    | MIDI and Key Remote                                           | 60        | M38       | Missing   |          |
| 3.21    | Saving and Exporting                                          | 60-61     | M08       | Prototype |          |
| 4.1     | Content Pane                                                  | 64        | M24       | Missing   |          |
| 4.2     | Search Bar                                                    | 65-70     | M24       | Missing   |          |
| 4.3     | Browser History                                               | 71        | M24       | Missing   |          |
| 4.4     | Filters and Tags                                              | 72-80     | M24       | Missing   |          |
| 4.5     | Collections                                                   | 81-82     | M24       | Missing   |          |
| 4.6     | Library                                                       | 83-84     | M24       | Missing   |          |
| 4.7     | Places                                                        | 85-116    | M24       | Missing   |          |
| 4.8     | Navigating in the Browser                                     | 117       | M24       | Missing   |          |
| 4.9     | Previewing Files                                              | 117-119   | M24       | Missing   |          |
| 4.10    | Adding Content from the Browser to a Live Set                 | 120-121   | M24       | Missing   |          |
| 5.1     | Sample Files                                                  | 122-132   | M11       | Missing   |          |
| 5.2     | MIDI Files                                                    | 133       | M10       | Missing   |          |
| 5.3     | Live Clips                                                    | 134       | M10       | Missing   |          |
| 5.4     | Live Sets                                                     | 135-141   | M09       | Missing   |          |
| 5.5     | Live Projects                                                 | 142-146   | M10       | Missing   |          |
| 5.6     | Locating Missing Files                                        | 147-149   | M10       | Missing   |          |
| 5.7     | Collecting External Files                                     | 150-154   | M10       | Missing   |          |
| 5.8     | Finding Unused Files                                          | 155-158   | M10       | Missing   |          |
| 5.9     | Packing Projects into Packs                                   | 159-162   | M10       | Missing   |          |
| 5.10    | File Management FAQs                                          | 163-164   | M10       | Missing   |          |
| 6.1     | Layout                                                        | 165-167   | M16       | Prototype |          |
| 6.2     | Navigation and Zooming                                        | 168       | M16       | Missing   |          |
| 6.3     | Transport and Playback                                        | 168-169   | M16       | Missing   |          |
| 6.4     | Launching the Arrangement with Locators                       | 170-171   | M16       | Missing   |          |
| 6.5     | Time Signature Changes                                        | 172-173   | M16       | Missing   |          |
| 6.6     | The Arrangement Loop                                          | 174       | M16       | Missing   |          |
| 6.7     | Moving and Resizing Clips                                     | 175-176   | M16       | Missing   |          |
| 6.8     | Audio Clip Fades and Crossfades                               | 177-178   | M16       | Missing   |          |
| 6.9     | Selecting Clips and Time                                      | 179       | M16       | Missing   |          |
| 6.10    | Using the Editing Grid                                        | 180       | M16       | Missing   |          |
| 6.11    | Using the Time Commands                                       | 181       | M16       | Missing   |          |
| 6.12    | Splitting Clips                                               | 181       | M16       | Prototype |          |
| 6.13    | Consolidating Clips                                           | 182       | M16       | Missing   |          |
| 6.14    | Linked-Track Editing                                          | 183-184   | M16       | Missing   |          |
| 6.15    | The Mixer in Arrangement View                                 | 185-186   | M16       | Missing   |          |
| 7.1     | Session View Clips                                            | 188       | M15       | Prototype |          |
| 7.2     | Tracks and Scenes                                             | 189-193   | M15       | Prototype |          |
| 7.3     | The Track Status Fields                                       | 194       | M15       | Missing   |          |
| 7.4     | Setting Up the Session View Grid                              | 195-196   | M15       | Missing   |          |
| 7.5     | Recording Sessions into the Arrangement                       | 197-199   | M15       | Missing   |          |
| 8.1     | Clip View Layout                                              | 201-206   | M17       | Missing   |          |
| 8.2     | Main Clip Properties Panel                                    | 207-211   | M17       | Missing   |          |
| 8.3     | Extended Clip Properties                                      | 212-213   | M17       | Missing   |          |
| 8.4     | Audio Utilities Panel                                         | 214-221   | M17       | Missing   |          |
| 8.5     | Pitch and Time Utilities Panel                                | 222-224   | M17       | Missing   |          |
| 8.6     | Transform and Generate Panels                                 | 225       | M19       | Missing   |          |
| 8.7     | Zooming and Scrolling in the Clip View's Editor               | 226       | M17       | Missing   |          |
| 8.8     | Playing and Scrubbing Clips                                   | 227       | M17       | Missing   |          |
| 8.9     | Looping Clips                                                 | 228-229   | M17       | Missing   |          |
| 8.10    | Clip View Sample Details                                      | 230       | M17       | Missing   |          |
| 8.11    | Cropping Clips                                                | 231       | M17       | Missing   |          |
| 8.12    | Replacing and Editing the Sample                              | 231       | M17       | Missing   |          |
| 8.13    | Editing Clip Properties for Multiple Clips                    | 232       | M17       | Missing   |          |
| 8.14    | Clip Defaults and Update Rate                                 | 232-233   | M17       | Missing   |          |
| 9.1     | Tempo                                                         | 234-237   | M23       | Missing   |          |
| 9.2     | Warping                                                       | 238-248   | M23       | Missing   |          |
| 9.3     | Warp Modes                                                    | 249-251   | M23       | Missing   |          |
| 10.1    | The MIDI Note Editor Layout                                   | 252-253   | M18       | Prototype |          |
| 10.2    | Zooming and Navigating in the MIDI Note Editor                | 254-256   | M18       | Missing   |          |
| 10.3    | Creating a MIDI Clip                                          | 257       | M18       | Missing   |          |
| 10.4    | Adding MIDI Notes                                             | 257-258   | M18       | Missing   |          |
| 10.5    | Editing MIDI Notes                                            | 259-283   | M18       | Missing   |          |
| 10.6    | Folding and Scales                                            | 284-286   | M18       | Prototype |          |
| 10.7    | Editing MIDI Clips                                            | 287-288   | M18       | Missing   |          |
| 10.8    | Multi-Clip Editing                                            | 289-292   | M18       | Missing   |          |
| 11.1    | Using MIDI Tools                                              | 293-296   | M19       | Missing   |          |
| 11.2    | Transformation Tools                                          | 297-320   | M19       | Prototype |          |
| 11.3    | Generative Tools                                              | 321-329   | M19       | Prototype |          |
| 12.1    | Viewing MPE Data                                              | 331       | M18       | Missing   |          |
| 12.2    | Editing MPE Data                                              | 332-334   | M18       | Missing   |          |
| 12.3    | Drawing Envelopes                                             | 335       | M18       | Missing   |          |
| 12.4    | MPE in Live's Devices and on Push 2                           | 336       | M18       | Missing   |          |
| 12.5    | MPE in External Plug-ins                                      | 336       | M18       | Missing   |          |
| 12.6    | MPE/Multi-channel Settings                                    | 336-339   | M18       | Missing   |          |
| 13.1    | Slice to New MIDI Track                                       | 340-342   | M36       | Missing   |          |
| 13.2    | Convert Harmony to New MIDI Track                             | 343       | M36       | Missing   |          |
| 13.3    | Convert Melody to New MIDI Track                              | 343       | M36       | Missing   |          |
| 13.4    | Convert Drums to New MIDI Track                               | 344       | M36       | Missing   |          |
| 13.5    | Optimizing for Better Conversion Quality                      | 344       | M36       | Missing   |          |
| 14.1    | Groove Pool                                                   | 346-348   | M35       | Missing   |          |
| 14.2    | Editing Grooves                                               | 349       | M35       | Missing   |          |
| 14.3    | Groove Tips                                                   | 350       | M35       | Missing   |          |
| 15.1    | Loading a Tuning System                                       | 353-355   | M35       | Prototype |          |
| 15.2    | The Tuning Section                                            | 356       | M35       | Missing   |          |
| 15.3    | MIDI Track Options for Tuning Systems                         | 357-358   | M35       | Missing   |          |
| 15.4    | Learn More About Tuning Systems                               | 359       | M35       | Missing   |          |
| 16.1    | The Launch Controls                                           | 360       | M07       | Missing   |          |
| 16.2    | Launch Modes                                                  | 361       | M15       | Missing   |          |
| 16.3    | Legato Mode                                                   | 362       | M15       | Missing   |          |
| 16.4    | Clip Launch Quantization                                      | 363       | M07       | Missing   |          |
| 16.5    | Velocity                                                      | 364       | M15       | Missing   |          |
| 16.6    | Clip Offset and Nudging                                       | 364-365   | M15       | Missing   |          |
| 16.7    | Follow Actions                                                | 366-371   | M15       | Missing   |          |
| 17.1    | Monitoring                                                    | 374       | M20       | Prototype |          |
| 17.2    | External Audio In/Out                                         | 375       | M06       | Missing   |          |
| 17.3    | External MIDI In/Out                                          | 375-379   | M06       | Missing   |          |
| 17.4    | Resampling                                                    | 380       | M20       | Missing   |          |
| 17.5    | Internal Routings                                             | 380-393   | M20       | Missing   |          |
| 18.1    | The Live Mixer                                                | 394-398   | M20       | Prototype |          |
| 18.2    | Audio and MIDI Tracks                                         | 399       | M20       | Missing   |          |
| 18.3    | Group Tracks                                                  | 400-401   | M20       | Missing   |          |
| 18.4    | Return Tracks and the Main track                              | 402       | M20       | Prototype |          |
| 18.5    | Using Live's Crossfader                                       | 403-405   | M20       | Missing   |          |
| 18.6    | Soloing and Cueing                                            | 406-407   | M20       | Missing   |          |
| 18.7    | Track Delays                                                  | 408-409   | M20       | Missing   |          |
| 18.8    | Keep Monitoring Latency in Recording Track Toggles            | 410       | M20       | Missing   |          |
| 18.9    | Performance Impact Track Indicators                           | 410-411   | M20       | Missing   |          |
| 19.1    | Choosing an Input                                             | 412       | M21       | Missing   |          |
| 19.2    | Arming (Record-Enabling) Tracks                               | 413       | M21       | Missing   |          |
| 19.3    | Recording                                                     | 413-416   | M21       | Missing   |          |
| 19.4    | Recording in Sync                                             | 417-418   | M21       | Missing   |          |
| 19.5    | Recording Quantized MIDI Notes                                | 419       | M21       | Missing   |          |
| 19.6    | Recording with Count-in                                       | 420       | M21       | Missing   |          |
| 19.7    | Setting up File Types                                         | 420       | M21       | Missing   |          |
| 19.8    | Where are the Recorded Samples?                               | 420       | M21       | Missing   |          |
| 19.9    | Using Remote Control for Recording                            | 421       | M21       | Missing   |          |
| 19.10   | Capturing MIDI                                                | 422-424   | M21       | Missing   |          |
| 20.1    | Bouncing Individual Tracks                                    | 425-426   | M36       | Missing   |          |
| 20.2    | Bouncing Group Tracks                                         | 427-428   | M36       | Missing   |          |
| 20.3    | Pasting Bounced Audio                                         | 429-430   | M36       | Missing   |          |
| 21.1    | Take Lanes                                                    | 431-432   | M21       | Missing   |          |
| 21.2    | Inserting and Managing Take Lanes                             | 433       | M21       | Missing   |          |
| 21.3    | Recording Takes                                               | 433       | M21       | Missing   |          |
| 21.4    | Inserting Samples                                             | 434       | M21       | Missing   |          |
| 21.5    | Auditioning Take Lanes                                        | 434       | M21       | Missing   |          |
| 21.6    | Creating a Comp                                               | 435       | M21       | Missing   |          |
| 21.7    | Source Highlights                                             | 436       | M21       | Missing   |          |
| 22.1    | How Stem Separation Works in Live                             | 437       | M36       | Missing   |          |
| 22.2    | Separating Audio Files and Clips                              | 437-444   | M36       | Missing   |          |
| 23.1    | Device View                                                   | 445-446   | M25       | Missing   |          |
| 23.2    | Using Devices                                                 | 447-476   | M25       | Missing   |          |
| 23.3    | Device Delay Compensation                                     | 477-478   | M05       | Missing   |          |
| 24.1    | Plug-Ins in the Device View                                   | 481-483   | M37       | Missing   |          |
| 24.2    | Sidechain Parameters                                          | 484       | M37       | Missing   |          |
| 24.3    | VST Plug-Ins                                                  | 485-488   | M37       | Missing   |          |
| 24.4    | Audio Units Plug-Ins                                          | 489-490   | M37       | Missing   |          |
| 25.1    | An Overview of Racks                                          | 491-492   | M25       | Missing   |          |
| 25.2    | Creating Racks                                                | 493       | M25       | Missing   |          |
| 25.3    | Looking at Racks                                              | 494       | M25       | Missing   |          |
| 25.4    | Chain List                                                    | 495-496   | M25       | Missing   |          |
| 25.5    | Zones                                                         | 497-500   | M25       | Missing   |          |
| 25.6    | Drum Racks                                                    | 501-503   | M32       | Prototype |          |
| 25.7    | Using the Macro Controls                                      | 504-507   | M25       | Missing   |          |
| 25.8    | Mixing With Racks                                             | 508-510   | M25       | Missing   |          |
| 26.1    | Recording Automation in Session View                          | 512       | M22       | Missing   |          |
| 26.2    | Recording Automation in Arrangement View                      | 513       | M22       | Missing   |          |
| 26.3    | Overwriting Automation                                        | 514       | M22       | Missing   |          |
| 26.4    | Overriding Automation                                         | 515       | M22       | Missing   |          |
| 26.5    | Viewing Automation                                            | 515-522   | M22       | Missing   |          |
| 26.6    | Creating Automation Envelopes                                 | 523-526   | M22       | Prototype |          |
| 26.7    | Editing Automation                                            | 527-529   | M22       | Missing   |          |
| 26.8    | Deleting Automation                                           | 530       | M22       | Missing   |          |
| 26.9    | Additional Tips for Working with Automation                   | 531-532   | M22       | Missing   |          |
| 27.1    | The Clip Envelope Editor                                      | 533       | M22       | Missing   |          |
| 27.2    | Audio Clip Envelopes                                          | 534-536   | M22       | Missing   |          |
| 27.3    | Mixer and Device Clip Envelopes                               | 537-541   | M22       | Missing   |          |
| 27.4    | MIDI Controller Clip Envelopes                                | 542       | M22       | Missing   |          |
| 27.5    | Unlinking Clip Envelopes From Clips                           | 542-545   | M22       | Missing   |          |
| 28.1    | Importing Video                                               | 546       | M39       | Missing   |          |
| 28.2    | The Appearance of Video in Live                               | 546-548   | M39       | Missing   |          |
| 28.3    | Matching Sound to Video                                       | 549       | M39       | Missing   |          |
| 28.4    | Video Trimming Tricks                                         | 549-551   | M39       | Missing   |          |
| 29.1    | Amp                                                           | 552-553   | M29       | Missing   |          |
| 29.2    | Auto Filter                                                   | 554-561   | M26       | Prototype |          |
| 29.3    | Auto Pan-Tremolo                                              | 562-565   | M30       | Prototype |          |
| 29.4    | Auto Shift                                                    | 566-573   | M30       | Missing   |          |
| 29.5    | Beat Repeat                                                   | 574-575   | M28       | Missing   |          |
| 29.6    | Cabinet                                                       | 576-577   | M29       | Missing   |          |
| 29.7    | Channel EQ                                                    | 578-579   | M26       | Missing   |          |
| 29.8    | Chorus-Ensemble                                               | 580-583   | M30       | Prototype |          |
| 29.9    | Compressor                                                    | 584-588   | M27       | Prototype |          |
| 29.10   | Corpus                                                        | 589-594   | M30       | Missing   |          |
| 29.11   | Delay                                                         | 595-598   | M28       | Prototype |          |
| 29.12   | Drum Buss                                                     | 599-600   | M27       | Missing   |          |
| 29.13   | Dynamic Tube                                                  | 601       | M29       | Missing   |          |
| 29.14   | Echo                                                          | 602-605   | M28       | Missing   |          |
| 29.15   | EQ Eight                                                      | 606-607   | M26       | Prototype |          |
| 29.16   | EQ Three                                                      | 608       | M26       | Prototype |          |
| 29.17   | Erosion                                                       | 609       | M29       | Missing   |          |
| 29.18   | External Audio Effect                                         | 610-611   | M26       | Missing   |          |
| 29.19   | Filter Delay                                                  | 612       | M28       | Missing   |          |
| 29.20   | Gate                                                          | 613-614   | M27       | Prototype |          |
| 29.21   | Glue Compressor                                               | 615-617   | M27       | Missing   |          |
| 29.22   | Grain Delay                                                   | 618       | M28       | Missing   |          |
| 29.23   | Hybrid Reverb                                                 | 619-625   | M28       | Missing   |          |
| 29.24   | Limiter                                                       | 626-627   | M27       | Prototype |          |
| 29.25   | Looper                                                        | 628-631   | M28       | Missing   |          |
| 29.26   | Multiband Dynamics                                            | 632-636   | M27       | Missing   |          |
| 29.27   | Overdrive                                                     | 637       | M29       | Missing   |          |
| 29.28   | Pedal                                                         | 638-640   | M29       | Missing   |          |
| 29.29   | Phaser-Flanger                                                | 641-643   | M30       | Missing   |          |
| 29.30   | Redux                                                         | 644-645   | M29       | Prototype |          |
| 29.31   | Resonators                                                    | 646       | M30       | Missing   |          |
| 29.32   | Reverb                                                        | 647-650   | M28       | Prototype |          |
| 29.33   | Roar                                                          | 651-659   | M29       | Missing   |          |
| 29.34   | Saturator                                                     | 660-662   | M29       | Prototype |          |
| 29.35   | Shifter                                                       | 663-667   | M30       | Missing   |          |
| 29.36   | Spectral Resonator                                            | 668-672   | M30       | Missing   |          |
| 29.37   | Spectral Time                                                 | 673-676   | M28       | Missing   |          |
| 29.38   | Spectrum                                                      | 677       | M26       | Missing   |          |
| 29.39   | Tuner                                                         | 678-683   | M26       | Missing   |          |
| 29.40   | Utility                                                       | 684-685   | M26       | Prototype |          |
| 29.41   | Vinyl Distortion                                              | 686       | M29       | Missing   |          |
| 29.42   | Vocoder                                                       | 687-690   | M30       | Missing   |          |
| 30.1    | Arpeggiator                                                   | 691-693   | M31       | Missing   |          |
| 30.2    | CC Control                                                    | 694       | M31       | Missing   |          |
| 30.3    | Chord                                                         | 695-696   | M31       | Missing   |          |
| 30.4    | Note Length                                                   | 697       | M31       | Missing   |          |
| 30.5    | Pitch                                                         | 698       | M31       | Missing   |          |
| 30.6    | Random                                                        | 699-700   | M31       | Missing   |          |
| 30.7    | Scale                                                         | 701       | M31       | Missing   |          |
| 30.8    | Velocity                                                      | 702-703   | M31       | Missing   |          |
| 31.1    | Analog                                                        | 704-712   | M33       | Missing   |          |
| 31.2    | Collision                                                     | 713-721   | M34       | Missing   |          |
| 31.3    | Drift                                                         | 722-729   | M33       | Missing   |          |
| 31.4    | Drum Sampler                                                  | 730-735   | M32       | Missing   |          |
| 31.5    | Electric                                                      | 736-740   | M34       | Missing   |          |
| 31.6    | External Instrument                                           | 741-742   | M34       | Missing   |          |
| 31.7    | Impulse                                                       | 743-744   | M32       | Missing   |          |
| 31.8    | Meld                                                          | 745-754   | M33       | Missing   |          |
| 31.9    | Operator                                                      | 755-772   | M33       | Prototype |          |
| 31.10   | Sampler                                                       | 773-790   | M32       | Missing   |          |
| 31.11   | Simpler                                                       | 791-801   | M32       | Prototype |          |
| 31.12   | Tension                                                       | 802-813   | M34       | Missing   |          |
| 31.13   | Wavetable                                                     | 814-823   | M33       | Missing   |          |
| 32.1    | Setting Up Max for Live                                       | 824       | M42       | Missing   |          |
| 32.2    | Using Max for Live Devices                                    | 825       | M42       | Missing   |          |
| 32.3    | Editing Max for Live Devices                                  | 825-827   | M42       | Missing   |          |
| 32.4    | Building Max for Live MIDI Tools                              | 828       | M42       | Missing   |          |
| 32.5    | Max Dependencies                                              | 829       | M42       | Missing   |          |
| 32.6    | Learning Max Programming                                      | 829       | M42       | Missing   |          |
| 33.1.1  | DS Clang                                                      | 831       | M31       | Missing   |          |
| 33.1.2  | DS Clap                                                       | 832       | M31       | Missing   |          |
| 33.1.3  | DS Cymbal                                                     | 833       | M31       | Missing   |          |
| 33.1.4  | DS FM                                                         | 834       | M31       | Missing   |          |
| 33.1.5  | DS HH                                                         | 835       | M31       | Missing   |          |
| 33.1.6  | DS Kick                                                       | 836       | M31       | Missing   |          |
| 33.1.7  | DS Snare                                                      | 837       | M31       | Missing   |          |
| 33.1.8  | DS Tom                                                        | 838       | M31       | Missing   |          |
| 33.2.1  | Align Delay                                                   | 839       | M31       | Missing   |          |
| 33.2.2  | Envelope Follower                                             | 840-842   | M31       | Missing   |          |
| 33.2.3  | LFO                                                           | 843-845   | M31       | Missing   |          |
| 33.2.4  | Shaper                                                        | 846-847   | M31       | Missing   |          |
| 33.3.1  | Envelope MIDI                                                 | 848-850   | M31       | Missing   |          |
| 33.3.2  | Expression Control                                            | 851-852   | M31       | Missing   |          |
| 33.3.3  | MIDI Monitor                                                  | 853-854   | M31       | Missing   |          |
| 33.3.4  | MPE Control                                                   | 855-860   | M31       | Missing   |          |
| 33.3.5  | Note Echo                                                     | 861       | M31       | Missing   |          |
| 33.3.6  | Shaper MIDI                                                   | 862-864   | M31       | Missing   |          |
| 34.1    | MIDI Remote Control                                           | 865-868   | M38       | Missing   |          |
| 34.2    | The Mapping Browser                                           | 869-873   | M38       | Missing   |          |
| 35.1    | Setup                                                         | 874       | M38       | Missing   |          |
| 35.2    | Browsing and Loading Sounds                                   | 875       | M38       | Missing   |          |
| 35.3    | Playing and Programming Beats                                 | 876-883   | M38       | Missing   |          |
| 35.4    | Additional Recording Options                                  | 884-885   | M38       | Missing   |          |
| 35.5    | Playing Melodies and Harmonies                                | 886-888   | M38       | Missing   |          |
| 35.6    | Step Sequencing Melodies and Harmonies                        | 889-891   | M38       | Missing   |          |
| 35.7    | Melodic Sequencer + 32 Notes                                  | 892       | M38       | Missing   |          |
| 35.8    | Navigating in Note Mode                                       | 893       | M38       | Missing   |          |
| 35.9    | Controlling Live's Instruments and Effects                    | 894       | M38       | Missing   |          |
| 35.10   | Mixing with Push 1                                            | 895-896   | M38       | Missing   |          |
| 35.11   | Recording Automation                                          | 897       | M38       | Missing   |          |
| 35.12   | Step Sequencing Automation                                    | 898       | M38       | Missing   |          |
| 35.13   | Controlling Live's Session View                               | 899-900   | M38       | Missing   |          |
| 35.14   | Setting User Preferences                                      | 901-902   | M38       | Missing   |          |
| 35.15   | Push 1 Control Reference                                      | 903-913   | M38       | Missing   |          |
| 36.1    | Setup                                                         | 915       | M38       | Missing   |          |
| 36.2    | Browsing and Loading Sounds                                   | 915-917   | M38       | Missing   |          |
| 36.3    | Playing and Programming Beats                                 | 918-929   | M38       | Missing   |          |
| 36.4    | Additional Recording Options                                  | 930-931   | M38       | Missing   |          |
| 36.5    | Playing Melodies and Harmonies                                | 932-935   | M38       | Missing   |          |
| 36.6    | Step Sequencing Melodies and Harmonies                        | 936-938   | M38       | Missing   |          |
| 36.7    | Melodic Sequencer + 32 Notes                                  | 939-941   | M38       | Missing   |          |
| 36.8    | Working with Samples                                          | 942-947   | M38       | Missing   |          |
| 36.9    | Navigating in Note Mode                                       | 948       | M38       | Missing   |          |
| 36.10   | Working With Instruments and Effects                          | 949-953   | M38       | Missing   |          |
| 36.11   | Track Control And Mixing                                      | 954-956   | M38       | Missing   |          |
| 36.12   | Recording Automation                                          | 957       | M38       | Missing   |          |
| 36.13   | Step Sequencing Automation                                    | 958       | M38       | Missing   |          |
| 36.14   | Clip Mode                                                     | 959-962   | M38       | Missing   |          |
| 36.15   | Controlling Live's Session View                               | 963-965   | M38       | Missing   |          |
| 36.16   | Setup Menu                                                    | 966       | M38       | Missing   |          |
| 36.17   | Push 2 Control Reference                                      | 967-972   | M38       | Missing   |          |
| 37.1    | Synchronizing via Link                                        | 973-977   | M38       | Missing   |          |
| 37.2    | Synchronizing via Tempo Follower                              | 978       | M38       | Missing   |          |
| 37.3    | Synchronizing via MIDI                                        | 979-981   | M38       | Missing   |          |
| 38.1    | Managing the CPU Load                                         | 982-986   | M41       | Missing   |          |
| 38.2    | Managing the Disk Load                                        | 987       | M11       | Missing   |          |
| 38.3    | The Overload Indicator                                        | 988       | M41       | Missing   |          |
| 39.1    | Testing and Methodology                                       | 989       | M05       | Missing   |          |
| 39.2    | Neutral Operations                                            | 989-992   | M05       | Missing   |          |
| 39.3    | Non-Neutral Operations                                        | 993-995   | M05       | Missing   |          |
| 39.4    | Tips for Achieving Optimal Sound Quality in Live              | 996       | M05       | Missing   |          |
| 39.5    | Conclusion                                                    | 996       | M05       | Missing   |          |
| 40.1    | Ideal MIDI Behavior                                           | 997       | M05       | Missing   |          |
| 40.2    | MIDI Timing Problems                                          | 998       | M05       | Missing   |          |
| 40.3    | Live's MIDI Solutions                                         | 998       | M05       | Missing   |          |
| 40.4    | Variables Outside of Live's Control                           | 999-1000  | M05       | Missing   |          |
| 40.5    | Tips for Achieving Optimal MIDI Performance                   | 1001      | M05       | Missing   |          |
| 40.6    | Summary and Conclusions                                       | 1001-1002 | M05       | Missing   |          |
| 41.1    | Menu and Keyboard Navigation Settings                         | 1003-1004 | M40       | Missing   |          |
| 41.2    | Audio Setup                                                   | 1005      | M40       | Missing   |          |
| 41.3    | Connecting MIDI Devices                                       | 1005      | M40       | Missing   |          |
| 41.4    | Navigating in Live                                            | 1006-1010 | M40       | Missing   |          |
| 41.5    | Editing Automation and Modulation Envelopes                   | 1011-1013 | M40       | Missing   |          |
| 42.1    | Showing and Hiding Views                                      | 1014      | M14       | Missing   |          |
| 42.2    | Keyboard Focus and Navigation                                 | 1015      | M14       | Missing   |          |
| 42.3    | Working with Sets and the Program                             | 1016      | M14       | Missing   |          |
| 42.4    | Working with Devices and Plug-Ins                             | 1017      | M14       | Missing   |          |
| 42.5    | Editing                                                       | 1017      | M14       | Missing   |          |
| 42.6    | Adjusting Values                                              | 1018      | M14       | Missing   |          |
| 42.7    | Commands for Breakpoint Envelopes                             | 1019      | M14       | Missing   |          |
| 42.8    | Loop Brace and Start/End Markers                              | 1020      | M14       | Missing   |          |
| 42.9    | Zooming, Display and Selections                               | 1020      | M14       | Missing   |          |
| 42.10   | Clip View Editor View Modes                                   | 1021      | M14       | Missing   |          |
| 42.11   | Clip View Sample Editor                                       | 1021      | M14       | Missing   |          |
| 42.12   | Clip View MIDI Note Editor                                    | 1022-1024 | M14       | Missing   |          |
| 42.13   | Grid Snapping and Drawing                                     | 1025      | M14       | Missing   |          |
| 42.14   | Global Quantization                                           | 1025      | M14       | Missing   |          |
| 42.15   | Session View                                                  | 1026      | M14       | Missing   |          |
| 42.16   | Arrangement View                                              | 1027-1028 | M14       | Missing   |          |
| 42.17   | Comping                                                       | 1029      | M14       | Missing   |          |
| 42.18   | Bounce to Audio                                               | 1029      | M14       | Missing   |          |
| 42.19   | Commands for Tracks                                           | 1030      | M14       | Missing   |          |
| 42.20   | Transport                                                     | 1031      | M14       | Missing   |          |
| 42.21   | Audio Engine                                                  | 1032      | M14       | Missing   |          |
| 42.22   | Browser                                                       | 1032      | M14       | Missing   |          |
| 42.23   | Similar Sample Swapping                                       | 1033      | M14       | Missing   |          |
| 42.24   | Key/MIDI Map Mode and the Computer MIDI Keyboard              | 1033      | M14       | Missing   |          |
| 42.25   | Momentary Latching Shortcuts                                  | 1034      | M14       | Missing   |          |
| 42.26   | General Keyboard Navigation and Workflow                      | 1034-1035 | M14       | Missing   |          |
| 42.27   | Editing Automation and Modulation Envelopes with the Keyboard | 1036      | M14       | Missing   |          |
| 42.28   | Accessing Menus                                               | 1036      | M14       | Missing   |          |
| 42.29   | Using Live's Context Menu                                     | 1036      | M14       | Missing   |          |

## Beyond

Rows for `BEYOND-LIVE.md` items, added when their milestone is expanded.

| ID  | Title                             | Milestone | Status  | Evidence |
| --- | --------------------------------- | --------- | ------- | -------- |
| B01 | Native Linux build                | M45       | Missing |          |
| B02 | Out-of-process plug-in hosting    | M37       | Missing |          |
| B03 | Headless CLI                      | M08       | Missing |          |
| B04 | Control server and MCP            | M42       | Missing |          |
| B05 | Scripted MIDI Tools               | M42       | Missing |          |
| B06 | Autosave and version history      | M43       | Missing |          |
| B07 | Output safety limiter             | M43       | Missing |          |
| B08 | Loudness and true-peak metering   | M44       | Missing |          |
| B09 | DAWproject import and export      | M44       | Missing |          |
| B10 | Version-control friendly sets     | M09       | Missing |          |
| B11 | Project health report             | M10       | Missing |          |
| B12 | Stem Separation in every build    | M36       | Missing |          |
| B13 | Deterministic renders with report | M08       | Missing |          |
| B14 | No account, no telemetry          | M45       | Missing |          |
