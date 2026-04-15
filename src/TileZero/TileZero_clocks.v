/*==============================================================================================================================

    Copyright © 2022..2025 Advanced Architectures

    All rights reserved
    Confidential Information
    Limited Distribution to Authorized Persons Only
    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

    Project Name         : Cognitum

    Description          : Programmable Clocks for Tile Zero

================================================================================================================================
    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
==============================================================================================================================*/
//
//===============================================================================================================================
//
//            31                                              7 6 5 4 3 2 1 0     All Oscillators
//           +-----------------------------------------------+-+-------+-----+
//           |                 read as zeros                 |E|   D   |  F  |    TCC,  NCC
//           +-----------------------------------------------+/+---/---+--/--+
//                    E:  Enable     Turn Oscillator on/off _/    /      /
//                    D:  Divisor     1..16 integer divide ______/      /
//                    F:  Frequency  16..23 GHz  ______________________/
//
//                                            1 1 1 1 1 1 1
//            31                              5 4 3 2 1 0 9 8 7 6 5 4 3 2 1 0     Raceway Secondary dividers
//           +-------------------------------+-+---+-----+---+-+-------+-----+
//           |         read as zeros         |E|SEL|TRIM |LG2|E|   D   |  F  |     RDC
//           +-------------------------------+/+-/-+--/--+-/-+-+-------+-----+
//          E: RESET    Assert reset ________/  /    /    /
//          S: Select   one of 4 stages _______/    /    /
//          T: Trim     divide by integer 1..8 ____/    /
//          L: Log2 Div divide by 2,4,8,16 ____________/
//
//
//           +-----NORTH-----+-----EAST------+-----WEST------+-----SOUTH-----+    LVDS Secondary dividers
//           |               |               |               |7 6 5 4 3 2 1 0|
//           +-+---+-----+---+-+---+-----+---+-+---+-----+---+-+---+-----+---+
//           |E|SEL|TRIM |LG2|E|SEL|TRIM |LG2|E|SEL|TRIM |LG2|E|SEL|TRIM |LG2|    NDC
//           +-+---+-----+---+-+---+-----+---+-+---+-----+---+/+-/-+--/--+-/-+
//                                           same as above___/__/____/____/ for each cardinal direction
//
//===============================================================================================================================

module   TileZero_clocks #(
   parameter            BASE        = 13'h00000,
   parameter            NUM_REGS    = 4
   )(
   // I/O Interface
   output wire [32:0]   imiso,
   input  wire [47:0]   imosi,
   output reg  [10:0]   TCC, //JLPL - 11_7_25 - Extend Width from 8 to 11 bits
   // Clocks
   output wire          north_clock,
   output wire          south_clock,
   output wire           east_clock,
   output wire           west_clock,
   output wire           race_clock,
   // Global
   output wire          race_reset,
   input  wire          ring_oscillator_reset,     // JLPL - 11_7_25
   input  wire          enable,
   input  wire          clock,
   input  wire          reset
   );

localparam  LG2REGS     =  `LG2(NUM_REGS);

`ifdef synthesis
`include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/COGNITUM_IO_addresses.vh"
`else
`include "COGNITUM_IO_addresses.vh"
`endif

// Register Fields
`define  OSC_ENABLE     7
`define  OSC_DIVIDE     6:4
`define  OSC_FREQ       2:0
`define  R_CLK_ENABLE  15
`define  R_CLK_SELECT  14:13
`define  R_DIV_TRIM    12:10
`define  R_DIV_LOG2     9:8
`define  N_CLK_ENABLE  31
`define  N_CLK_SELECT  30:29
`define  N_DIV_TRIM    28:26
`define  N_DIV_LOG2    25:24
`define  E_CLK_ENABLE  23
`define  E_CLK_SELECT  22:21
`define  E_DIV_TRIM    20:18
`define  E_DIV_LOG2    17:16
`define  W_CLK_ENABLE  15
`define  W_CLK_SELECT  14:13
`define  W_DIV_TRIM    12:10
`define  W_DIV_LOG2     9:8
`define  S_CLK_ENABLE   7
`define  S_CLK_SELECT   6:5
`define  S_DIV_TRIM     4:2
`define  S_DIV_LOG2     1:0

wire        race_osc,         news_osc;
wire        race_base_clock,  news_base_clock;
wire        race_trim_clock,  north_trim_clock, east_trim_clock,  west_trim_clock,  south_trim_clock;

reg  [ 7:0] NCC;
reg  [15:0] RDC;
reg  [31:0] NDC;

reg  [ 3:0] RDIV,  NDIV,  EDIV,  WDIV,  SDIV;
reg         RDIV2, NDIV2, EDIV2, WDIV2, SDIV2;

wire        news_reset;
wire        race_log2_clock, north_log2_clock, east_log2_clock, west_log2_clock, south_log2_clock;

// Interface decoding -----------------------------------------------------------------------------------------------------------

wire  [31:0]   iIO, IOo;
wire  [12:0]   aIO;
wire  [ 2:0]   cIO;
// Decollate imosi
assign   {cIO, iIO, aIO} = imosi;

wire                  in_range =  aIO[12:LG2REGS]  == BASE[12:LG2REGS];
wire  [NUM_REGS-1:0]    decode =  in_range << aIO[LG2REGS-1:0];
wire  [NUM_REGS-1:0]     write =  {NUM_REGS{(cIO==3'b110)}} & decode;
wire  [NUM_REGS-1:0]      read =  {NUM_REGS{(cIO==3'b101)}} & decode;

// Writes
wire
      ld_TCC =  write[ZCKS_TILE_CTRL],  // TileZero Clock
      ld_RDC =  write[ZCKS_RACE_CTRL],  // Raceway Divider & Clock
      ld_NCC =  write[ZCKS_NEWS_CTRL],  // Base NEWS Frequency
      ld_NDC =  write[ZCKS_NEWS_DIVD];  // Dividers for all 4 cardinals
// Reads
wire  rd_TCC =   read[ZCKS_TILE_CTRL],  //
      rd_RDC =   read[ZCKS_RACE_CTRL],  //
      rd_NCC =   read[ZCKS_NEWS_CTRL],  //
      rd_NDC =   read[ZCKS_NEWS_DIVD];  //

//always @(posedge clock) if (reset || ld_TCC) TCC <= `tCQ reset ?  11'h0f8     : iIO[10:0]; //JLPL - 11_7_25 - F8 Default

//JLPL - 11_7_25
always @(posedge clock or posedge ring_oscillator_reset)
	begin
		if (ring_oscillator_reset) TCC <= `tCQ 11'h0f8;
		else if  (ld_TCC) TCC <= `tCQ iIO[10:0];
	end //end always
//End JLPL - 11_7_25

always @(posedge clock) if (reset || ld_RDC) RDC <= `tCQ reset ? `RDC_default : iIO[15:0];
always @(posedge clock) if (reset || ld_NCC) NCC <= `tCQ reset ? `NCC_default : iIO[07:0];
always @(posedge clock) if (reset || ld_NDC) NDC <= `tCQ reset ? `NDC_default : iIO[31:0];

assign IOo  =  rd_NDC ?  NDC[31:0]
            :  rd_NCC ? {24'h0,NCC[ 7:0]}
            :  rd_RDC ? {16'h0,RDC[ 7:0]}
            :  rd_TCC ? {24'h0,TCC[ 7:0]}
            :  32'h0;

wire  IOk  = (|write[NUM_REGS-1:0]) | (|read[NUM_REGS-1:0]);

// Collate output
assign imiso = {IOk, IOo[31:0]};

//===============================================================================================================================

// Oscillators
`ifdef   synthesis
wire [2:0] RaceOsc_ctrl, NEWSOsc_ctrl;
wire RaceOsc_en, NEWSOsc_en ;
assign RaceOsc_ctrl = RDC[`OSC_FREQ] ;
assign NEWSOsc_ctrl = NCC[`OSC_FREQ] ;
assign RaceOsc_en = enable & RDC[`OSC_ENABLE];
assign NEWSOsc_en = enable & NCC[`OSC_ENABLE];
RingOsc_A9_C14 iRaceOsc (.clock(race_osc), .d2(RaceOsc_ctrl[2]), .d1(RaceOsc_ctrl[1]), .d0(RaceOsc_ctrl[0]), .EN(RaceOsc_en));
RingOsc_A9_C14 iNEWSOsc (.clock(news_osc), .d2(NEWSOsc_ctrl[2]), .d1(NEWSOsc_ctrl[1]), .d0(NEWSOsc_ctrl[0]), .EN(NEWSOsc_en));
`else

A2_RingOscillator iRaceOsc (.RO(race_osc), .ctrl(RDC[`OSC_FREQ]), .enable(enable & RDC[`OSC_ENABLE]));
A2_RingOscillator iNEWSOsc (.RO(news_osc), .ctrl(NCC[`OSC_FREQ]), .enable(enable & NCC[`OSC_ENABLE]));

`endif

// Resets
A2_reset u_rst_n (.q(news_reset), .reset(reset), .clock(news_osc));   // Synchronized to Individual clock
// Primary dividers
A2_Johnson_Count16 iRaceDiv (.div_clock(race_base_clock), .ref_clock(race_osc), .divider(RDC[6:3]), .reset(reset));
A2_Johnson_Count16 iNEWSDiv (.div_clock(news_base_clock), .ref_clock(news_osc), .divider(NCC[6:3]), .reset(reset));

// Secondary counter-dividers
always @(posedge race_base_clock) if (race_reset || RDC[15]) RDIV <= `tCQ race_reset ? 4'h0 : (RDIV + 1'b1);
always @(posedge news_base_clock) if (news_reset || NDC[31]) NDIV <= `tCQ news_reset ? 4'h0 : (NDIV + 1'b1);
always @(posedge news_base_clock) if (news_reset || NDC[23]) EDIV <= `tCQ news_reset ? 4'h0 : (EDIV + 1'b1);
always @(posedge news_base_clock) if (news_reset || NDC[15]) WDIV <= `tCQ news_reset ? 4'h0 : (WDIV + 1'b1);
always @(posedge news_base_clock) if (news_reset || NDC[ 7]) SDIV <= `tCQ news_reset ? 4'h0 : (SDIV + 1'b1);

// Secondary counter-dividers select (select divides by 1, 2, 4 or 8)
assign
    race_log2_clock  =  RDIV >> RDC[ 9: 8],
   north_log2_clock  =  RDIV >> NDC[ 1: 0],
    east_log2_clock  =  RDIV >> NDC[ 9: 8],
    west_log2_clock  =  RDIV >> NDC[17:16],
   south_log2_clock  =  RDIV >> NDC[25:24];

A2_Johnson_Count8 iRtrim (.div_clock( race_trim_clock),.ref_clock( race_log2_clock),.divider(RDC[12:10]),.reset(race_reset)); //JLPL - 9_14_25 - Remove Parameter
A2_Johnson_Count8 iNtrim (.div_clock(north_trim_clock),.ref_clock(north_log2_clock),.divider(NDC[28:26]),.reset(news_reset)); //JLPL - 9_14_25 - Remove Parameter
A2_Johnson_Count8 iEtrim (.div_clock( east_trim_clock),.ref_clock( east_log2_clock),.divider(NDC[20:18]),.reset(news_reset)); //JLPL - 9_14_25 - Remove Parameter
A2_Johnson_Count8 iWtrim (.div_clock( west_trim_clock),.ref_clock( west_log2_clock),.divider(NDC[12:10]),.reset(news_reset)); //JLPL - 9_14_25 - Remove Parameter
A2_Johnson_Count8 iStrim (.div_clock(south_trim_clock),.ref_clock(south_log2_clock),.divider(NDC[ 4:2 ]),.reset(news_reset)); //JLPL - 9_14_25 - Remove Parameter

always @(posedge  race_trim_clock) if (race_reset           ) RDIV2 <= `tCQ race_reset ? 4'h0 : ~RDIV2;
always @(posedge north_trim_clock) if (news_reset || NDC[31]) NDIV2 <= `tCQ news_reset ? 4'h0 : ~NDIV2;
always @(posedge  east_trim_clock) if (news_reset || NDC[23]) EDIV2 <= `tCQ news_reset ? 4'h0 : ~EDIV2;
always @(posedge  west_trim_clock) if (news_reset || NDC[15]) WDIV2 <= `tCQ news_reset ? 4'h0 : ~WDIV2;
always @(posedge south_trim_clock) if (news_reset || NDC[ 7]) SDIV2 <= `tCQ news_reset ? 4'h0 : ~SDIV2;

assign
    race_clock =            {race_base_clock, race_log2_clock, race_trim_clock,RDIV2} >> RDC[14:13],
   north_clock = NDC[31] & ({news_base_clock,north_log2_clock,north_trim_clock,NDIV2} >> NDC[30:29]),
    east_clock = NDC[23] & ({news_base_clock, east_log2_clock, east_trim_clock,EDIV2} >> NDC[22:21]),
    west_clock = NDC[15] & ({news_base_clock, west_log2_clock, west_trim_clock,WDIV2} >> NDC[14:13]),
   south_clock = NDC[07] & ({news_base_clock,south_log2_clock,south_trim_clock,SDIV2} >> NDC[ 6:5 ]);

A2_reset i_rrs (.q(race_reset), .reset(reset | RDC[15]), .clock(race_clock));


endmodule
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//
//    Johnson Ring Counters:
//         16        15        14       13       12      11      10     9      8     7     6    5    4   3   2
//      0  00000000            0000000           000000          00000         0000        000       00      0
//      1  00000001  00000001  0000001  0000001  000001  000001  00001  00001  0001  0001  001  001  01  01  1
//      2  00000011  00000011  0000011  0000011  000011  000011  00011  00011  0011  0011  011  011  11  11
//      3  00000111  00000111  0000111  0000111  000111  000111  00111  00111  0111  0111  111  111  10  10
//      4  00001111  00001111  0001111  0001111  001111  001111  01111  01111  1111  1111  110  110
//      5  00011111  00011111  0011111  0011111  011111  011111  11111  11111  1110  1110  100  100
//      6  00111111  00111111  0111111  0111111  111111  111111  11110  11110  1100  1100
//      7  01111111  01111111  1111111  1111111  111110  111110  11100  11100  1000  1000
//      8  11111111  11111111  1111110  1111110  111100  111100  11000  11000
//      9  11111110  11111110  1111100  1111100  111000  111000  10000  10000
//      10 11111100  11111100  1111000  1111000  110000  110000
//      11 11111000  11111000  1110000  1110000  100000  100000
//      12 11110000  11110000  1100000  1100000
//      13 11100000  11100000  1000000  1000000
//      14 11000000  11000000
//      15 10000000  10000000
//
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

module A2_Johnson_Count16 (
   output wire       div_clock,     // Resulting divided down clock
   input  wire       ref_clock,     // Reference clock to be divided down
   input  wire [3:0] divider,       // Divide ref_clock by 1..16
   input  wire       reset          // Reset synchronoue with clock
   );

localparam [255:0]  K1  = {
   //   Clock-En Feedback                     mark:space
   16'b1111111101111111,    // divide by 16    8:8     50.00% //JL
   16'b1111111100111111,    // divide by 15    8:7     53.33% //JL
   16'b0111111110111111,    // divide by 14    7:7     50.00% //JL
   16'b0111111110011111,    // divide by 13    7:6     53.84% //JL
   16'b0011111111011111,    // divide by 12    6:6     50.00% //JL
   16'b0011111111001111,    // divide by 11    6:5     54.54% //JL
   16'b0001111111101111,    // divide by 10    5:5     50.00% //JL
   16'b0001111111100111,    // divide by 9     5:4     55.55% //JL
   16'b0000111111110111,    // divide by 8     4:4     50.00% //JL
   16'b0000111111110011,    // divide by 7     4:3     57.14% //JL
   16'b0000011111111011,    // divide by 6     3:3     50.00% //JL
   16'b0000011111111001,    // divide by 5     3:2     60.00% //JL
   16'b0000001111111101,    // divide by 4     2:2     50.00% //JL
   16'b0000001111111100,    // divide by 3     2:1     66.66% //JL
   16'b0000000111111110,    // divide by 2     1:1     50.00% //JL
   16'b0000000011111111     // off
   };

// Level 1 Divider  -------------------------------------------------------------------------------------------------------------

wire  [15:0]   ckctrl = K1[16*divider[3:0]+:16];
reg   [ 7:0]   ring;

always @(posedge ref_clock or posedge reset)
   if    (reset     )   ring[7:0] <= #1  8'hff;
   else begin
      if (ckctrl[08])   ring[0]   <= #1 ~&(ring[7:0] | ckctrl[7:0]);
      if (ckctrl[09])   ring[1]   <= #1    ring[0];
      if (ckctrl[10])   ring[2]   <= #1    ring[1];
      if (ckctrl[11])   ring[3]   <= #1    ring[2];
      if (ckctrl[12])   ring[4]   <= #1    ring[3];
      if (ckctrl[13])   ring[5]   <= #1    ring[4];
      if (ckctrl[14])   ring[6]   <= #1    ring[5];
      if (ckctrl[15])   ring[7]   <= #1    ring[6];
      end

assign  div_clock =  ring[0];

endmodule

//JLSW - Break into 2 different modules - 2nd module
module A2_Johnson_Count8 (
   output wire       div_clock,     // Resulting divided down clock
   input  wire       ref_clock,     // Reference clock to be divided down
   input  wire [2:0] divider,       // Divide ref_clock by 1..8
   input  wire       reset          // Reset synchronoue with clock
   );

localparam [63:0]  K2  = {
   // Clk-En Fb
   8'b11110111, // divide by 8
   8'b11110011, // divide by 7
   8'b01111011, // divide by 6
   8'b01111001, // divide by 5
   8'b00111101, // divide by 4
   8'b00111100, // divide by 3
   8'b00011110, // divide by 2
   8'b00011110  // off  //JL
   };

// Level 1 Divider  -------------------------------------------------------------------------------------------------------------

wire  [ 7:0]   ckctrl = K2[8*divider[2:0]+:8]; //JLPL - 11_9_25
reg   [ 3:0]   ring;

always @(posedge ref_clock)
   if    (reset     )   ring[3:0] <= #1  4'hf;
   else begin
      if (ckctrl[04])   ring[0]   <= #1 ~&(ring[3:0] | ckctrl[3:0]);
      if (ckctrl[05])   ring[1]   <= #1    ring[0];
      if (ckctrl[06])   ring[2]   <= #1    ring[1];
      if (ckctrl[07])   ring[3]   <= #1    ring[2];
      end

assign  div_clock =  ring[0]; //JL

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

`ifdef   verify_clockDiv

`timescale  1ns/1ps

module t;

reg   [3:0] divider;
reg         ref_clock;
reg         reset;

wire        div_clock;
integer     i;

A2_Johnson_Counter mut (
   .div_clock  (div_clock),
   .ref_clock  (ref_clock),
   .divider    (divider  ),
   .reset      (reset)
   );


initial begin
   ref_clock = 0;
   forever
      ref_clock = #(43.4782608696) ~ref_clock;
   end

initial begin
   divider  =  0;
   reset    =  0;
   #100;
   reset    =  1;
   #5000;
   reset    =  0;

   for (i=0; i<32; i=i+1) begin
      repeat (64)  @(posedge ref_clock);
      divider = (divider + 1) %16;
      end

   #500;

   $stop;
   $finish;
   end

endmodule
`endif
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
