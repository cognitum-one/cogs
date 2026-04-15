//===============================================================================================================================
//
//  Copyright © 2021 Advanced Architectures
//
//  All rights reserved
//  Confidential Information
//  Limited Distribution to Authorized Persons Only
//  Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//  Project Name         : Scrypt ASIC
//
//  Description          : Raceway
//                         One section of the interconnect system that connects all the Scrypt Tiles together and interfaces
//                         with the I/O sections.
//                         This module connects a column of 8 tiles to the Raceway and to Raceways either side.
//
//===============================================================================================================================
//  THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//  IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//  BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//  TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//  AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//  ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//         +--------+--------+--------+--------+--------------------------------+--------------------------------+
//         |command |  tag   |  dest  | source |          write data 0          |             address            |
//         +--------+--------+------v-+-v------+--------------------------------+--------------------------------+
//                                   \ /
//                                    X
//                                   / \
//         +--------+--------+------v-+-v------+--------------------------------+--------------------------------+
//         |  ackn. |  tag   |  dest  | source |          read data 0           |           read data 1          |
//         +--------+--------+--------+--------+--------------------------------+--------------------------------+
//
//                                97       96     95:88      87:80      79:72      71:64      63:32       31:0
//    INPUTS:              98 = { xresetn, xpush, xcmd[7:0], xtag[7:0], xdst[7:0], xsrc[7:0], xwrd[31:0], xadr[31:0] }
//    outputs:             98 = { xtmo,    xordy, xack[7:0], xtag[7:0], xsrc[7:0], xdst[7:0], xrd0[31:0], xrd1[31:0] }
//
//===============================================================================================================================

`ifndef  tCQ
`define  tCQ   #1
`endif

module Raceway #(
   parameter   [  7:3]  colID = 0
   )(
   // Raceway Output                //       95:88     87:80     79:72     71:64     63:32      32:0
   output wire [  3:0]  CO_or,      //
   output wire [383:0]  CO,         //  96 { cmd[7:0], tag[7:0], dst[7:0], src[7:0], wrd[31:0], adr[31:0]}
   input  wire [  3:0]  CO_rd,      //
   output wire          CO_ck,      //  No Pipe on output side -- connects to raceway in next column\hub
   // Raceway Input
   input  wire [  3:0]  CI_wr,      //
   input  wire [383:0]  CI,         //  96 { cmd[7:0], tag[7:0], dst[7:0], src[7:0], wrd[31:0], adr[31:0]}
   output wire [  3:0]  CI_ir,      //
   input  wire          CI_ck,      //
   // Local Port Output Side
   output wire [783:0]  RT,         //    Eight xmosi munged together
   input  wire [  7:0]  RT_fb,      //
   output wire [  7:0]  RT_ck,      //
   // Local Port Input Side
   input  wire [783:0]  TR,         //    Eight xmiso munged together
   output wire [  7:0]  TR_fb,      //
   input  wire [  7:0]  TR_ck,      //
   // Global
   input  wire          reset       //
   );

genvar   i;
integer  j,k;

//===============================================================================================================================
// Locals =======================================================================================================================
//===============================================================================================================================

wire  [11:0]   in_or;               // Crossbar Input from input pipes 'output ready'
wire  [95:0]   in       [0:11];
wire  [11:0]   in_rd;               // Crossbar take (pop) current data from the pipe

wire  [11:0]   in_sel   [0:11];     // Crossbar Select vectors

wire  [11:0]   co_ir;               // Crossbar Output from output pipes 'input ready'
wire  [95:0]   co       [0:11];     // Crossbar Output data
wire  [11:0]   co_wr;               // Crossbar give (push) current selected data to the pipe

wire [143:0]   in_ack, in_ct_ack;

wire  [95:0]   home_decode, port_requests, lane8, lane9;
wire  [ 7:0]   away_req_decode, away_rsp_decode;
wire  [ 1:0]   rw2rw_req_pass, rw2rw_rsp_pass;

wire  [63:0]   tp2tp_req;
wire  [15:0]   rw2tp_req, rw2tp_rsp;

wire  [63:0]   tp2tp_win;
wire  [15:0]   rw2tp_req_win, rw2tp_rsp_win;

wire  [15:0]   tp2rw_req_win, tp2rw_rsp_win;

wire           tp2tp_adv      =  1'b1,
               rw2tp_rsp_adv  =  1'b1,
               rw2tp_req_adv  =  1'b1,
               tp2rw_rsp_adv  =  1'b1,
               tp2rw_req_adv  =  1'b1,
               broadcast_adv  =  1'b1;

wire  [ 7:0]   broadcast_new,    // 'n'-hot source port to distribute
               broadcast_win,    // one-hit after arbitration
               broadcast_end;    // one-hot destination port to finish
wire           broadcast_run,    // forward request lane-input to outputs AND distribute to all local tile ports
               broadcast;

//===============================================================================================================================
// CLOCKS & RESETS ==============================================================================================================
//===============================================================================================================================

wire     clock =  CI_ck;            // Column clock in
assign   CO_ck =  clock;            // Column clock out

for (i=0;i<8;i=i+1)  begin : A      // Distribute clocks and resets to the Tiles in this column
   assign
      RT[98*i+97] = ~reset,         // 97 195 293 391 489 587 685 783
      RT_ck[i]    =  clock;
   end

//===============================================================================================================================
// DATAPATH =====================================================================================================================
//===============================================================================================================================

`define  PRIORI   A2_pipe_priority
`define  CR       .clock(clock),.reset(reset)                                         //          +--not the same!!--+
`PRIORI #(96) iA1 (.por(in_or[ 0]),.pdo(in[ 0][95:0]),.prd(in_rd[ 0]),.pir(TR_fb[0]),.pdi(TR[98*0+:96]),.pwr(TR[98*0+ 96]), `CR);
`PRIORI #(96) iA2 (.por(in_or[ 1]),.pdo(in[ 1][95:0]),.prd(in_rd[ 1]),.pir(TR_fb[1]),.pdi(TR[98*1+:96]),.pwr(TR[98*1+ 96]), `CR);
`PRIORI #(96) iA3 (.por(in_or[ 2]),.pdo(in[ 2][95:0]),.prd(in_rd[ 2]),.pir(TR_fb[2]),.pdi(TR[98*2+:96]),.pwr(TR[98*2+ 96]), `CR);
`PRIORI #(96) iA4 (.por(in_or[ 3]),.pdo(in[ 3][95:0]),.prd(in_rd[ 3]),.pir(TR_fb[3]),.pdi(TR[98*3+:96]),.pwr(TR[98*3+ 96]), `CR);
`PRIORI #(96) iA5 (.por(in_or[ 4]),.pdo(in[ 4][95:0]),.prd(in_rd[ 4]),.pir(TR_fb[4]),.pdi(TR[98*4+:96]),.pwr(TR[98*4+ 96]), `CR);
`PRIORI #(96) iA6 (.por(in_or[ 5]),.pdo(in[ 5][95:0]),.prd(in_rd[ 5]),.pir(TR_fb[5]),.pdi(TR[98*5+:96]),.pwr(TR[98*5+ 96]), `CR);
`PRIORI #(96) iA7 (.por(in_or[ 6]),.pdo(in[ 6][95:0]),.prd(in_rd[ 6]),.pir(TR_fb[6]),.pdi(TR[98*6+:96]),.pwr(TR[98*6+ 96]), `CR);
`PRIORI #(96) iA8 (.por(in_or[ 7]),.pdo(in[ 7][95:0]),.prd(in_rd[ 7]),.pir(TR_fb[7]),.pdi(TR[98*7+:96]),.pwr(TR[98*7+ 96]), `CR);

wire wrA9 = CI_wr[0];   // Strange bug!!

A2_pipe #(96) iA9 (.por(in_or[ 8]),.pdo(in[ 8][95:0]),.prd(in_rd[ 8]),.pir(CI_ir[0]),.pdi(CI[96*0+:96]),.pwr(wrA9    ),     `CR);
A2_pipe #(96) iA10(.por(in_or[ 9]),.pdo(in[ 9][95:0]),.prd(in_rd[ 9]),.pir(CI_ir[1]),.pdi(CI[96*1+:96]),.pwr(CI_wr[1]),     `CR);
A2_pipe #(96) iA11(.por(in_or[10]),.pdo(in[10][95:0]),.prd(in_rd[10]),.pir(CI_ir[2]),.pdi(CI[96*2+:96]),.pwr(CI_wr[2]),     `CR);
A2_pipe #(96) iA12(.por(in_or[11]),.pdo(in[11][95:0]),.prd(in_rd[11]),.pir(CI_ir[3]),.pdi(CI[96*3+:96]),.pwr(CI_wr[3]),     `CR);

// BROADCAST END ----------------------------------------------------------------------------------------------------------------
assign                          // clear the msb (request-bit) and swap the source and destination fields!
   lane8[95:0] = |broadcast_end ? {1'b0,in[8][94:80],in[8][71:64],in[8][79:72],in[8][63:0]} : in[8][95:0],
   lane9[95:0] = |broadcast_end ? {1'b0,in[9][94:80],in[9][71:64],in[9][79:72],in[9][63:0]} : in[9][95:0];

// CROSSBAR CONNEXIONS ----------------------------------------------------------------------------------------------------------
for (i=0;i<12;i=i+1) begin : B
   A2_mux #(12,96) iP (
      .z(co[i]),
      .s(in_sel[i]),
      .d({in[11],in[10],lane9,lane8,in[7],in[6],in[5],in[4],in[3],in[2],in[1],in[0]})
      );
   end

// TILE PORT OUTPUTS ------------------------------------------------------------------------------------------------------------
A2_pipe #(96) iB1 (.por(RT[98*0+96]),.pdo(RT[98*0+:96]),.prd(RT_fb[0]),.pir(co_ir[0]),.pdi(co[0][95:0]),.pwr(co_wr[0]), `CR);
A2_pipe #(96) iB2 (.por(RT[98*1+96]),.pdo(RT[98*1+:96]),.prd(RT_fb[1]),.pir(co_ir[1]),.pdi(co[1][95:0]),.pwr(co_wr[1]), `CR);
A2_pipe #(96) iB3 (.por(RT[98*2+96]),.pdo(RT[98*2+:96]),.prd(RT_fb[2]),.pir(co_ir[2]),.pdi(co[2][95:0]),.pwr(co_wr[2]), `CR);
A2_pipe #(96) iB4 (.por(RT[98*3+96]),.pdo(RT[98*3+:96]),.prd(RT_fb[3]),.pir(co_ir[3]),.pdi(co[3][95:0]),.pwr(co_wr[3]), `CR);
A2_pipe #(96) iB5 (.por(RT[98*4+96]),.pdo(RT[98*4+:96]),.prd(RT_fb[4]),.pir(co_ir[4]),.pdi(co[4][95:0]),.pwr(co_wr[4]), `CR);
A2_pipe #(96) iB6 (.por(RT[98*5+96]),.pdo(RT[98*5+:96]),.prd(RT_fb[5]),.pir(co_ir[5]),.pdi(co[5][95:0]),.pwr(co_wr[5]), `CR);
A2_pipe #(96) iB7 (.por(RT[98*6+96]),.pdo(RT[98*6+:96]),.prd(RT_fb[6]),.pir(co_ir[6]),.pdi(co[6][95:0]),.pwr(co_wr[6]), `CR);
A2_pipe #(96) iB8 (.por(RT[98*7+96]),.pdo(RT[98*7+:96]),.prd(RT_fb[7]),.pir(co_ir[7]),.pdi(co[7][95:0]),.pwr(co_wr[7]), `CR);

// LANE OUTPUTS------------------------------------------------------------------------------------------------------------------
assign   CO [383:0]  = {co[11][95:0], co[10][95:0], co[9][95:0], co[8][95:0]},
         CO_or[3:0]  =  co_wr[11:8];

//===============================================================================================================================
// DECODES ======================================================================================================================
//===============================================================================================================================

//  \\\\\\\\
//  >Nota Bene: for local 'home' accesses there is no differentiating of requests and responses. Not needed and simpler >>>>>>>-
//  ////////

// Any of the 12 inputs carrying data for any of the 8 tile ports
for (i=0;i<12;i=i+1) begin : C
   assign
      home_decode[8*i+:8]  =  {8{in_or[i]}} & (in[i][79:75] == colID[7:3]) <<  in[i][74:72];
   end

// Any of the  8 tile ports carrying data for tiles in other columns,  requests & responses
for (i=0;i<8; i=i+1) begin : D
   assign
      away_req_decode[i]   =  in_or[i]  &  in[i][95] & (in[i][79:75] != colID[7:3]),
      away_rsp_decode[i]   =  in_or[i]  & ~in[i][95] & (in[i][79:75] != colID[7:3]);
   end

// Passthrough lanes that do NOT address this column
assign
   rw2rw_req_pass[0]       =  in_or[8]  &  in[ 8][95] & (in[ 8][79:75] != colID[7:3]),
   rw2rw_req_pass[1]       =  in_or[9]  &  in[ 9][95] & (in[ 9][79:75] != colID[7:3]),
   rw2rw_rsp_pass[0]       =  in_or[10] & ~in[10][95] & (in[10][79:75] != colID[7:3]),
   rw2rw_rsp_pass[1]       =  in_or[11] & ~in[11][95] & (in[11][79:75] != colID[7:3]);

// Broadcasts on 10 inputs (NOT on the 2 response lanes)
// Tile Inputs (7:0) initiate broadcasts to all other tiles in this column and send out in request lanes (9:8)
// Lane Inputs (9:8) check for broadcast end and responed only to the broadcast initiator ELSE forward to ALL outputs (9:0)
for (i=0;i<8;i=i+1) begin : E
   assign    broadcast_new[i] =  in_or[i] & ((in[i][95:88] == 8'b10110001) | (in[i][95:88] == 8'b10100000) | (in[i][95:88] == 8'b10111000));                      // Need to arbitrate for a winner
   end

assign
   broadcast_run  =    in_or[8]   &  ((in[8][95:88] == 8'b10110001) | (in[8][95:88] == 8'b10100000) | (in[8][95:88] == 8'b10111000)) & (in[8][71:67] != colID[7:3])    // Only from lanes 8 or 9
                  |    in_or[9]   &  ((in[9][95:88] == 8'b10110001) | (in[9][95:88] == 8'b10100000) | (in[9][95:88] == 8'b10111000)) & (in[9][71:67] != colID[7:3]),

   broadcast_end  = {8{in_or[8]}} & (((in[8][95:88] == 8'b10110001) | (in[8][95:88] == 8'b10100000) | (in[8][95:88] == 8'b10111000)) & (in[8][71:67] == colID[7:3])) << in[8][66:64]  // detect
                  | {8{in_or[9]}} & (((in[9][95:88] == 8'b10110001) | (in[9][95:88] == 8'b10100000) | (in[9][95:88] == 8'b10111000)) & (in[9][71:67] == colID[7:3])) << in[9][66:64]; //  pre-flip!!

//===============================================================================================================================
// TRANSPOSE DECODES ============================================================================================================
//===============================================================================================================================
//
// input   76543210_76543210_76543210_76543210_76543210_76543210_76543210_76543210_76543210_76543210_76543210_76543210
// output  __777777777777_666666666666_555555555555_444444444444_333333333333_222222222222_111111111111_000000000000__
//
for (i=0;i<96;i=i+1) begin : F
   assign port_requests[i] =  home_decode[(i%12)*8+(i/12)];
   end
// Decollate
for (i=0;i<8;i=i+1) begin : G
   assign
      tp2tp_req[i*8 +: 8]   = port_requests[i*12    +:8],
      rw2tp_req[i*2 +: 2]   = port_requests[i*12+8  +:2],
      rw2tp_rsp[i*2 +: 2]   = port_requests[i*12+10 +:2];
   end

//===============================================================================================================================
// ARBITRATION ==================================================================================================================
//===============================================================================================================================
// Select a new broadcast from 1 of 8 tile ports
A2_arbiter #(8) iRB1 (
   .win(broadcast_win[7:0]),
   .req(broadcast_new[7:0]),
   .adv(broadcast_adv),
   .en (1'b1),
   .clock(clock),
   .reset(reset)
   );

// Tile Port Output Selects ------------------
for (i=0;i<8;i=i+1) begin : H
   // Port to port
   A2_arbiter #(8) iRB1 (
      .win(tp2tp_win[8*i+:8]),
      .req(tp2tp_req[8*i+:8]),
      .adv(tp2tp_adv),
      .en (1'b1),
      .clock(clock),
      .reset(reset)
      );
   // Lane Request to port
   A2_arbiter #(2) iRB2 (
      .win(rw2tp_req_win[2*i+:2]),
      .req(rw2tp_req[2*i+:2]),
      .adv(rw2tp_req_adv),
      .en (1'b1),
      .clock(clock),
      .reset(reset)
      );
   // Lane Response to port
   A2_arbiter #(2) iRB3 (
      .win(rw2tp_rsp_win[2*i+:2]),
      .req(rw2tp_rsp[2*i+:2]),
      .adv(rw2tp_rsp_adv),
      .en (1'b1),
      .clock(clock),
      .reset(reset)
      );
   end

// Raceway Port Output Selects ------------------
// Now the fun starts !!
// For Tile port to raceway assign 2 lanes to requests and 2 lanes to responses and keep them separate
// Requests
A2_arbiter_for2 #(8) iRB4 (
   .winner1(tp2rw_req_win[ 7: 0]),
   .winner2(tp2rw_req_win[15: 8]),
   .request(away_req_decode[7:0]),
   .enable (1'b1),
   .advance(tp2rw_req_adv),
   .clock  (clock),
   .reset  (reset)
   );

A2_arbiter_for2 #(8) iRB5 (
   .winner1(tp2rw_rsp_win[ 7: 0]),
   .winner2(tp2rw_rsp_win[15: 8]),
   .request(away_rsp_decode[7:0]),
   .enable (1'b1),
   .advance(tp2rw_rsp_adv),
   .clock  (clock),
   .reset  (reset)
   );
//                         INPUTS
//          <---lane---><---------port--------->
//           v  v  v  v  v  v  v  v  v  v  v  v
//           |  |  |  |  |  |  |  |  |  |  |  |
//     11   -O--+--+--+--O--O--O--O--O--O--O--O---->     lane  \
//           |  |  |  |  |  |  |  |  |  |  |  |                 +---  Raceway Responses
//     10   -+--O--+--+--O--O--O--O--X--O--O--O---->     lane  /
//           |  |  |  |  |  |  |  |  |  |  |  |
//      9   -+--+--R--+--O--O--O--O--X--O--O--O---->     lane  \
//           |  |  |  |  |  |  |  |  |  |  |  |                 +---  Raceway Requests
//      8   -+--+--+--O--O--O--O--O--X--O--O--O---->     lane  /
//           |  |  |  |  |  |  |  |  |  |  |  |
//      7   -O--O--R--O--O--O--O--O--X--O--O--O---->  O  port  \
//           |  |  |  |  |  |  |  |  |  |  |  |       U         |
//      6   -O--O--R--O--O--O--O--O--X--O--O--\---->  T  port   |
//           |  |  |  |  |  |  |  |  |  |  |  |       P         |
//      5   -O--O--R--O--O--O--O--O--X--O--O--O---->  U  port   |
//           |  |  |  |  |  |  |  |  |  |  |  |       T         |
//      4   -O--O--R--O--O--O--O--O--X--O--O--O---->  S  port   |
//           |  |  |  |  |  |  |  |  |  |  |  |                 >---  Column
//      3   -O--O--R--E--O--O--O--O--X--O--O--O---->     port   |
//           |  |  |  |  |  |  |  |  |  |  |  |                 |
//      2   -O--O--R--O--O--O--O--O--X--O--O--O---->     port   |
//           |  |  |  |  |  |  |  |  |  |  |  |                 |
//      1   -O--O--R--O--O--O--O--O--X--O--O--O---->     port   |    X = BROADCAST NEW
//           |  |  |  |  |  |  |  |  |  |  |  |                 |    R = BROADCAST RUN
//      0   -O--O--R--O--O--O--O--O--X--O--O--O---->     port  /     E = BROADCAST END
//
//          11 10  9  8  7  6  5  4  3  2  1  0

wire  [3:0] caseA [0:11];      // for debug only
// Prefer Raceway over Locals
assign // col row
   {caseA[11][3:0],
   in_sel[11][11:0]} =   rw2rw_rsp_pass[1]      ?  {4'd1, 12'b1000_0000_0000}             // bits to set in vertical columns
                     :   rw2rw_rsp_pass[0]      ?  {4'd2,  4'b0000, tp2rw_rsp_win[ 7:0]}
                     :                             {4'd3,  4'b0000, tp2rw_rsp_win[15:8]},

   {caseA[10][3:0],
   in_sel[10][11:0]} =   rw2rw_rsp_pass[0]      ?  {4'd1, 12'b0100_0000_0000}
                     :                             {4'd2,  4'b0000, tp2rw_rsp_win[ 7:0]},

   {caseA[ 9][3:0],
   in_sel[ 9][11:0]} =   broadcast_end          ?  {4'd1, 12'b0000_0000_0000}
                     :   broadcast_run          ?  {4'd2, 12'b0011_0000_0000}
                     :  |broadcast_new          ?  {4'd3,  4'b0000, broadcast_win[ 7:0]}
                     :   rw2rw_req_pass[1]      ?  {4'd4, 12'b0010_0000_0000}
                     :   rw2rw_req_pass[0]      ?  {4'd5,  4'b0000, tp2rw_req_win[ 7:0]}
                     :                             {4'd6,  4'b0000, tp2rw_req_win[15:8]},

   {caseA[ 8][3:0],
   in_sel[ 8][11:0]} =   broadcast_end          ?  {4'd1, 12'b0000_0000_0000}
                     :   broadcast_run          ?  {4'd2, 12'b0011_0000_0000}
                     :   broadcast_new          ?  {4'd3,  4'b0000, broadcast_win[ 7:0]}
                     :   rw2rw_req_pass[0]      ?  {4'd4, 12'b0001_0000_0000}
                     :                             {4'd5,  4'b0000, tp2rw_req_win[ 7:0]};

for (i=0;i<8;i=i+1)  begin : I  assign
  {caseA[i][3:0],
   in_sel[i][11:0]}  =  |rw2tp_rsp_win[2*i+:2]  ?  {4'd1,           rw2tp_rsp_win[2*i+:2],   10'b00_0000_0000}
                     :  |broadcast_new          ?  {4'd2,  4'b0000, broadcast_win[ 7:0]}
                     :   broadcast_run          ?  {4'd3, 12'b0011_0000_0000}
                     :   broadcast_end[i]       ?  {4'd4, 12'b0011_0000_0000}
                     :  |broadcast_end          ?  {4'd7, 12'b0000_0000_0000}    //JLPL - block any other write if broadcast end is present but not in this i iteration
                     :  |rw2tp_req_win[2*i+:2]  ?  {4'd5,  2'b00,   rw2tp_req_win[2*i+:2],       8'b0000_0000}
                     :                             {4'd6,  4'b0000, tp2tp_win    [8*i+:8]                    };
   end
   
//JLPL
wire [3:0] test_caseA0;
wire [3:0] test_caseA1;
assign test_caseA0 = caseA[0];
assign test_caseA1 = caseA[1];
//End JLPL

//===============================================================================================================================
// CREATE QUALIFIED WRITES TO OUTPUT PIPES ======================================================================================
//===============================================================================================================================

assign
   co_ir[11:8] =  CO_rd[ 3:0],
   all_ir      = &co_ir[11:0],
   broadcast   = |broadcast_new || broadcast_run ;

for (i=0;i<12; i=i+1) begin : J
   assign   co_wr[i] = broadcast ? all_ir & |in_sel[i][11:0] : |in_sel[i][11:0];
   end

assign
   CO_or[3:0]  = co_wr[11:8];

//===============================================================================================================================
// DE-MULTIPLEX ACKNOWLEDGE RESPONSES ===========================================================================================
//===============================================================================================================================

// De-multiplex to create acknowledges
for (i=0;i<12; i=i+1) begin : K
   assign   in_ack[12*i+:12]  =  {12{co_ir[i]}} & in_sel[i][11:0];
   end

// Transpose back to input format
for (i=0;i<144;i=i+1) begin : L
   assign   in_ct_ack[i] = in_ack[(i%12)*12+(i/12)];
   end

// Or 'em up to create the reads
for (i=0;i<12; i=i+1) begin : N
   assign   in_rd[i] =  broadcast  ? all_ir & |in_ct_ack[12*i+:12] : |in_ct_ack[12*i+:12];
   end

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

`ifdef   verify_raceway

module t;

// Raceway Output         //          95:88     87:80     79:72     71:64     63:32      32:0
wire [  3:0]  CO_or;      //
wire [383:0]  CO;         // 4 x 96 { cmd[7:0], tag[7:0], dst[7:0], src[7:0], wrd[31:0], adr[31:0]}
reg  [  3:0]  CO_rd;      //
wire          CO_ck;
// Raceway Input
reg  [  3:0]  CI_wr;
reg  [383:0]  CI;
wire [  3:0]  CI_ir;
reg           CI_ck;
// Local Port Output Side
wire [783:0]  RT;         // Eight xmosi munged together
reg  [  7:0]  RT_fb;
// Local Port Input Side
reg  [783:0]  TR;         // Eight xmiso munged together
wire [  7:0]  TR_fb;
wire [  7:0]  TR_timo;
// Global
reg           reset;

Raceway #(0) mut (
   .CO_or   (CO_or),      //
   .CO      (CO),         //  96 { cmd[7:0], tag[7:0], dst[7:0], src[7:0], wrd[31:0], adr[31:0]}
   .CO_rd   (CO_rd),      //
   .CO_ck   (CO_ck),
   .CI_wr   (CI_wr),
   .CI      (CI),
   .CI_ir   (CI_ir),
   .CI_ck   (CI_ck),
   .RT      (RT),         // Eight xmosi munged together
   .RT_fb   (RT_fb),
   .TR      (TR),         // Eight xmiso munged together
   .TR_fb   (TR_fb),
   .reset   (reset)
   );

initial begin
   CO_rd [  3:0]  =   4'h0;
   CI_wr [  3:0]  =   4'h0;
   CI    [383:0]  = 384'h0;
   CI_ck          =   1'b0;
   RT_fb [  7:0]  =   8'h0;
   TR    [783:0]  = 784'h0;
   reset          =   1'b0;

   $finish;
   $stop;
   end

endmodule
`endif
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
`ifdef STUFF

   0000     0 - - -
   0001     0 - - -
   0010     - 0 - -
   0011     0 1 - -
   0100     - - 0 -
   0101     0 - 1 -
   0110     - 0 1 -
   0111     0 1 2 -
   1000     - - - 0
   1001     0 - - 1
   1010     - 0 - 1
   1011     0 1 - 2
   1100     - - 0 1
   1101     0 - 1 2
   1110     - 0 1 2
   1111     0 1 2 3



`endif
