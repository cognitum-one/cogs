`timescale 1ns / 1ps    //Joel 1_31_24
/*==============================================================================================================================

  Copyright © 2021 Advanced Architectures

  All rights reserved
  Confidential Information
  Limited Distribution to Authorized Persons Only
  Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

  Project Name         : Scrypt ASIC

  Description          : Raceway Hub
                         Two Hubs are required to connect left and Right halves of a row and a column to interconnect the
                         two sets of array rows.

=================================================================================================================================
  THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
  IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
  BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
  TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
  AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
  ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
============================================================================================================================== */
//
// INPUTS:                                                                                                        FEEDBACK
// push-----+                                                                                                          +--> irdy
// reset    |                                                                                                          |
// clock\   |                                                                                                          |
//     \ \  v                                                                                                          |
//     +-+-+-+----8---+----8---+----8---+----8---+---------------32---------------+----------------32--------------+  +-+
//     |c|r|p|command |  tag   |  dest  | source |          write data 0          |             address            |  | |
// ====+-+-+-+--------+--------+--------X--------+--------------------------------+--------------------------------+==+-+========
//     |c|r|o|command |  tag   |  dest  | source |          read data 0           |             read data 1        |  | |
//     +-+-+-+--------+--------+--------+--------+--------------------------------+--------------------------------+  +-+
//          |                                                                                                          ^
//          |                                                                                                          |
//          +---> ordy                                                                                        pop ->---+
// OUTPUTS:
//
//==============================================================================================================================*/

/*==============================================================================================================================
Broadcast Flow : 

1) Broadcast Request has higher priority than a Normal Request or Response Transaction
2) Broadcast Requests enter the Hub as a matched pair -> Raceway Lanes 0 & 1
3) Broadcast Input Arbitration is Limited to Evaluating Left Channel 0, Right Channel 0 & Center Channel 0
4) All incoming Broadcast Requests are placed in a queue (bcast_id) where the tag and channel are stored
5) The Serviced Broadcast Request is then blocked to create another request
6) The incoming Broadcast Request in the Pipe is Taken after the other 2 side Broadcast Requests are sent to appropriate other channels ( i.e. - Left Incoming causes Right Outgoing and Center Outgoing Broadcast Requests)
7) The Broadcast action state machine uses the Broadcast_State_Register signal to control the state machine Flow
8) The queued Brodcast Request is cleared when the appropriate other channels finish their loop and respond with another Broadcast request to the Hub input Channel ( i.e. - Right and Center Incoming send Broadcast requests )
9) The source side Broadcast Loop is then completed by sending a Broadcast request out of the Hub channel that initiated this activity ( i.e. - Left Outgoing Broadcast request finishes the loop in the Hub )

Note : 

Signal any_tag_match ->  = '1' when a Hub Channel Response matches a previously queued Hub Input Broadcast Request
Signal any_tag_match ->  = '0' when this is an initial Hub input Broadcast Request

//==============================================================================================================================*/

// include for the timespec for simulation 
`ifdef synthesis //JLSW
	 `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
`include "A2_project_settings.vh"
`endif
//End JL  

//`define  CKRS   .clock(clock),.reset(reset)

module Hub (
   // Raceway Left Side Outgoing
   output wire [383:0]  WHO,    //***
   output wire [  3:0]  WHO_or, //***
   input  wire [  3:0]  WHO_rd, //***
   output wire          WHO_ck, //***
   output wire          WHO_reset, //***
   // Raceway Left Side Incoming
   input  wire [383:0]  WHI, //***
   input  wire [  3:0]  WHI_wr, //***
   output wire [  3:0]  WHI_ir, //***
   input  wire          WHI_ck, //***
   input  wire          WHI_reset, //***
   // Raceway Right Side Outgoing
   output wire [383:0]  EHO,
   output wire [  3:0]  EHO_or,
   input  wire [  3:0]  EHO_rd,
   output wire          EHO_ck,
   output wire          EHO_reset, //***   
   // Raceway Right Side Incoming
   input  wire [383:0]  EHI,
   input  wire [  3:0]  EHI_wr,
   output wire [  3:0]  EHI_ir,
   input  wire          EHI_ck,
   input  wire          EHI_reset, //***   
   // Raceway Column Outgoing
   output wire  [383:0]  HO, //***
   output wire  [  3:0]  HO_or, //***
   input wire   [  3:0]  HO_rd, //***
   output wire        HO_ck,     //***
   output wire        HO_reset,  //*** 
   // Raceway Column Incoming
   input wire  [383:0]  HI, //***
   input wire  [  3:0]  HI_wr, //***
   output wire [  3:0]  HI_ir, //***
   input wire        HI_ck,     //***
   input wire        HI_reset,  //***
   // Global
   input  wire        ID,     // 0 = This module is the NORTH Hub, 1 = This module is the SOUTH hub
   input  wire        reset,
   input  wire        clock
   );

localparam [0:0]  FALSE = 1'b0, TRUE = 1'b1;

// Inputs =======================================================================================================================
// 12 Pipe inputs with FIFO controls

wire  [95:0]   li0, li1, li2, li3,     // Internal Raceway lane inputs
               ri0, ri1, ri2, ri3,
               ci0, ci1, ci2, ci3;
wire  [ 3:0]   li_ordy,  li_take,      // Internal give(ordy) and take for each lane
               ri_ordy,  ri_take,
               ci_ordy,  ci_take;
			   

wire [95:0] ls0;
wire [95:0] ls1;
wire [95:0] ls2;
wire [95:0] ls3;
wire [95:0] rs0;
wire [95:0] rs1;
wire [95:0] rs2;
wire [95:0] rs3;
wire [95:0] cs0;
wire [95:0] cs1;
wire [95:0] cs2;
wire [95:0] cs3;

//===============================================================================================================================
// CORE INTERFACE TRANSLATION LAYER =============================================================================================
//===============================================================================================================================
   wire [383:0]  L_QO;
   wire [  3:0]  L_QO_or;
   wire [  3:0]  L_QO_rd;
   wire [383:0]  L_QI;
   wire [  3:0]  L_QI_wr;
   wire [  3:0]  L_QI_ir;
   assign WHO    = L_QO; //New Module Interface Output Name
   assign WHO_or = L_QO_or; //New Module Interface Output Name
   assign L_QO_rd = WHO_rd; //New Module Interface Input Name
   assign L_QI = WHI; //New Module Interface Input Name
   assign L_QI_wr = WHI_wr; //New Module Interface Input Named
   assign WHI_ir = L_QI_ir; //New Module Interface Output Named     

   wire [383:0]  R_QO;
   wire [  3:0]  R_QO_or;
   wire [  3:0]  R_QO_rd;
   wire [383:0]  R_QI;
   wire [  3:0]  R_QI_wr;
   wire [  3:0]  R_QI_ir;
   assign EHO    = R_QO; //New Module Interface Output Name
   assign EHO_or = R_QO_or; //New Module Interface Output Name
   assign R_QO_rd = EHO_rd; //New Module Interface Input Name
   assign R_QI = EHI; //New Module Interface Input Name
   assign R_QI_wr = EHI_wr; //New Module Interface Input Named
   assign EHI_ir = R_QI_ir; //New Module Interface Output Named 
   
   wire [96:0] Co0;
   wire [96:0] Co1;
   wire [96:0] Co2;
   wire [96:0] Co3;

   assign HO[95:0] = Co0[95:0]; //New Module Interface Output Name
   assign HO_or[0] = Co0[96]; //New Module Interface Output Name
   assign HO[191:96] = Co1[95:0]; //New Module Interface Output Name
   assign HO_or[1] = Co1[96]; //New Module Interface Output Name
   assign HO[287:192] = Co2[95:0]; //New Module Interface Output Name
   assign HO_or[2] = Co2[96]; //New Module Interface Output Name
   assign HO[383:288] = Co3[95:0]; //New Module Interface Output Name
   assign HO_or[3] = Co3[96]; //New Module Interface Output Name
   
   wire Co0_fb;
   wire Co1_fb;
   wire Co2_fb;
   wire Co3_fb;
   
   assign Co0_fb = HO_rd[0]; //New Module Interface Input Name
   assign Co1_fb = HO_rd[1]; //New Module Interface Input Name
   assign Co2_fb = HO_rd[2]; //New Module Interface Input Name
   assign Co3_fb = HO_rd[3]; //New Module Interface Input Name
   
   wire [96:0] Ci0;
   wire [96:0] Ci1;
   wire [96:0] Ci2;
   wire [96:0] Ci3;

   assign Ci0[95:0] = HI[95:0]; //New Module Interface Input Name
   assign Ci0[96] = HI_wr[0]; //New Module Interface Input Name
   assign Ci1[95:0] = HI[191:96]; //New Module Interface Input Name
   assign Ci1[96] = HI_wr[1]; //New Module Interface Input Name
   assign Ci2[95:0] = HI[287:192]; //New Module Interface Input Name
   assign Ci2[96] = HI_wr[2]; //New Module Interface Input Name
   assign Ci3[95:0] = HI[383:288]; //New Module Interface Input Name
   assign Ci3[96] = HI_wr[3]; //New Module Interface Input Name
   
   wire Ci0_fb;
   wire Ci1_fb;
   wire Ci2_fb;
   wire Ci3_fb;
   
   assign HI_ir[0] = Ci0_fb; //New Module Interface Output Name
   assign HI_ir[1] = Ci1_fb; //New Module Interface Output Name
   assign HI_ir[2] = Ci2_fb; //New Module Interface Output Name
   assign HI_ir[3] = Ci3_fb; //New Module Interface Output Name

//===============================================================================================================================
// CLOCKS & RESETS SYNCRONISER ==================================================================================================
//===============================================================================================================================

wire reset_clock;

A2_reset rst_clock    (.q(reset_clock),   .reset(reset), .clock(clock    ));   // Synchronized to Raceway Logic Input Clock


// Deskew incoming clocks =======================================================================================================
`define  CKRSL   .clock(WHI_ck),.reset(WHI_reset)  //Incoming reset is skew delayed to be synchronous to rising edge of WHI_ck //***
`define  CKRSR   .clock(EHI_ck),.reset(EHI_reset)  //Incoming reset is skew delayed to be synchronous to rising edge of R_QI_ck
`define  CKRSC   .clock(HI_ck),.reset(HI_reset)   //Incoming reset is skew delayed to be synchronous to rising edge of Ci_clock
`define  CKRS    .clock(clock),.reset(reset_clock)   //Synchronise external reset input to incoming raceway_clock (signal clock)

//===============================================================================================================================
// CLOCKS & RESETS OUT ==============================================================================================================
//===============================================================================================================================
assign WHO_ck = clock; //***
assign WHO_reset = reset_clock; //***
assign EHO_ck = clock; //***
assign EHO_reset = reset_clock; //***
assign HO_ck = clock;
assign HO_reset = reset_clock; //***

//===============================================================================================================================
// JL - Declare Earlier for NcVerilog Compile Errors ============================================================================
//===============================================================================================================================
wire li0_take_comb;
wire li1_take_comb;
wire li2_take_comb;
wire li3_take_comb;
wire ri0_take_comb;
wire ri1_take_comb;
wire ri2_take_comb;
wire ri3_take_comb;
wire ci0_take_comb;
wire ci1_take_comb;
wire ci2_take_comb;
wire ci3_take_comb;
wire li0_take;
wire li1_take;
wire li2_take;
wire li3_take;
wire ri0_take;
wire ri1_take;
wire ri2_take;
wire ri3_take;
wire ci0_take;
wire ci1_take;
wire ci2_take;
wire ci3_take;


// Raceway Channel Left Inputs 
A2_pipe_priority_broadcast #(96) i_ppl1  (.pdi(L_QI[95:0]),.pwr(L_QI_wr[0]),.pir(L_QI_ir[0]), .pdo(ls0[95:0]),.por(ls0_ordy),.prd(li0_fb), `CKRSL);
A2_pipe_priority_broadcast #(96) i_ppl2  (.pdi(L_QI[191:96]),.pwr(L_QI_wr[1]),.pir(L_QI_ir[1]), .pdo(ls1[95:0]),.por(ls1_ordy),.prd(li1_fb), `CKRSL);
A2_pipe_priority_broadcast #(96) i_ppl3  (.pdi(L_QI[287:192]),.pwr(L_QI_wr[2]),.pir(L_QI_ir[2]), .pdo(ls2[95:0]),.por(ls2_ordy),.prd(li2_fb), `CKRSL);
A2_pipe_priority_broadcast #(96) i_ppl4  (.pdi(L_QI[383:288]),.pwr(L_QI_wr[3]),.pir(L_QI_ir[3]), .pdo(ls3[95:0]),.por(ls3_ordy),.prd(li3_fb), `CKRSL);
 
// Raceway Channel Right Inputs
A2_pipe_priority_broadcast #(96) i_ppl5  (.pdi(R_QI[95:0]),.pwr(R_QI_wr[0]),.pir(R_QI_ir[0]), .pdo(rs0[95:0]),.por(rs0_ordy),.prd(ri0_fb), `CKRSR);
A2_pipe_priority_broadcast #(96) i_ppl6  (.pdi(R_QI[191:96]),.pwr(R_QI_wr[1]),.pir(R_QI_ir[1]), .pdo(rs1[95:0]),.por(rs1_ordy),.prd(ri1_fb), `CKRSR);
A2_pipe_priority_broadcast #(96) i_ppl7  (.pdi(R_QI[287:192]),.pwr(R_QI_wr[2]),.pir(R_QI_ir[2]), .pdo(rs2[95:0]),.por(rs2_ordy),.prd(ri2_fb), `CKRSR);
A2_pipe_priority_broadcast #(96) i_ppl8  (.pdi(R_QI[383:288]),.pwr(R_QI_wr[3]),.pir(R_QI_ir[3]), .pdo(rs3[95:0]),.por(rs3_ordy),.prd(ri3_fb), `CKRSR);

// Raceway Channel Column Inputs
A2_pipe_priority_broadcast #(96) i_ppl9  (.pdi(Ci0[95:0]),.pwr(Ci0[96]),.pir(Ci0_fb), .pdo(cs0[95:0]),.por(cs0_ordy),.prd(ci0_fb), `CKRSC);
A2_pipe_priority_broadcast #(96) i_ppl10 (.pdi(Ci1[95:0]),.pwr(Ci1[96]),.pir(Ci1_fb), .pdo(cs1[95:0]),.por(cs1_ordy),.prd(ci1_fb), `CKRSC);
A2_pipe_priority_broadcast #(96) i_ppl11 (.pdi(Ci2[95:0]),.pwr(Ci2[96]),.pir(Ci2_fb), .pdo(cs2[95:0]),.por(cs2_ordy),.prd(ci2_fb), `CKRSC); 
A2_pipe_priority_broadcast #(96) i_ppl12 (.pdi(Ci3[95:0]),.pwr(Ci3[96]),.pir(Ci3_fb), .pdo(cs3[95:0]),.por(cs3_ordy),.prd(ci3_fb), `CKRSC);

// Fully synchronous and In-phase Inputs ========================================================================================
// Raceway Channel Left Inputs
A2_pipe_priority_broadcast #(96) i_ppl13 (.pdi(ls0[95:0]),.pwr(ls0_ordy),.pir(li0_fb), .pdo(li0[95:0]),.por(li0_ordy),.prd(li0_take_comb),`CKRS);
A2_pipe_priority_broadcast #(96) i_ppl14 (.pdi(ls1[95:0]),.pwr(ls1_ordy),.pir(li1_fb), .pdo(li1[95:0]),.por(li1_ordy),.prd(li1_take_comb),`CKRS);
A2_pipe_priority_broadcast #(96) i_ppl15 (.pdi(ls2[95:0]),.pwr(ls2_ordy),.pir(li2_fb), .pdo(li2[95:0]),.por(li2_ordy),.prd(li2_take_comb),`CKRS);
A2_pipe_priority_broadcast #(96) i_ppl16 (.pdi(ls3[95:0]),.pwr(ls3_ordy),.pir(li3_fb), .pdo(li3[95:0]),.por(li3_ordy),.prd(li3_take_comb),`CKRS);

// Raceway Channel Right Inputs
A2_pipe_priority_broadcast #(96) i_ppl17  (.pdi(rs0[95:0]),.pwr(rs0_ordy),.pir(ri0_fb), .pdo(ri0[95:0]),.por(ri0_ordy),.prd(ri0_take_comb),`CKRS);
A2_pipe_priority_broadcast #(96) i_ppl18  (.pdi(rs1[95:0]),.pwr(rs1_ordy),.pir(ri1_fb), .pdo(ri1[95:0]),.por(ri1_ordy),.prd(ri1_take_comb),`CKRS);
A2_pipe_priority_broadcast #(96) i_ppl19  (.pdi(rs2[95:0]),.pwr(rs2_ordy),.pir(ri2_fb), .pdo(ri2[95:0]),.por(ri2_ordy),.prd(ri2_take_comb),`CKRS);
A2_pipe_priority_broadcast #(96) i_ppl20  (.pdi(rs3[95:0]),.pwr(rs3_ordy),.pir(ri3_fb), .pdo(ri3[95:0]),.por(ri3_ordy),.prd(ri3_take_comb),`CKRS);

// Raceway Channel Column Inputs
A2_pipe_priority_broadcast #(96) i_ppl21 (.pdi(cs0[95:0]),.pwr(cs0_ordy),.pir(ci0_fb), .pdo(ci0[95:0]),.por(ci0_ordy),.prd(ci0_take_comb),`CKRS);
A2_pipe_priority_broadcast #(96) i_ppl22 (.pdi(cs1[95:0]),.pwr(cs1_ordy),.pir(ci1_fb), .pdo(ci1[95:0]),.por(ci1_ordy),.prd(ci1_take_comb),`CKRS);
A2_pipe_priority_broadcast #(96) i_ppl23 (.pdi(cs2[95:0]),.pwr(cs2_ordy),.pir(ci2_fb), .pdo(ci2[95:0]),.por(ci2_ordy),.prd(ci2_take_comb),`CKRS);
A2_pipe_priority_broadcast #(96) i_ppl24 (.pdi(cs3[95:0]),.pwr(cs3_ordy),.pir(ci3_fb), .pdo(ci3[95:0]),.por(ci3_ordy),.prd(ci3_take_comb),`CKRS);

reg [2:0] Broadcast_State_Register = 3'b000;
reg bcast_stall = 1'b0;
reg bcast_block = 1'b0;
reg [7:0] bcast_id [11:0];
reg [3:0] arbitration_port_number;
wire li0_bcast;
wire li1_bcast;
wire li2_bcast;
wire li3_bcast;
wire ri0_bcast;
wire ri1_bcast;
wire ri2_bcast;
wire ri3_bcast;
wire ci0_bcast;
wire ci1_bcast;
wire ci2_bcast;
wire ci3_bcast;
wire bcast_stall_comb;
wire bcast_block_comb;
wire li0_normal;
wire li1_normal;
wire li2_normal;
wire li3_normal;
wire ri0_normal;
wire ri1_normal;
wire ri2_normal;
wire ri3_normal;
wire ci0_normal;
wire ci1_normal;
wire ci2_normal;
wire ci3_normal;
wire bcast_stall_switch;
assign bcast_stall_switch = bcast_stall | bcast_stall_comb;
wire bcast_stall_switch1;
assign bcast_stall_switch1 = bcast_stall | bcast_stall_comb;
reg [2:0] li0_breq = 3'h0;
reg [2:0] li1_breq = 3'h0;
reg [2:0] li2_breq = 3'h0;
reg [2:0] li3_breq = 3'h0;
reg [2:0] ri0_breq = 3'h0;
reg [2:0] ri1_breq = 3'h0;
reg [2:0] ri2_breq = 3'h0;
reg [2:0] ri3_breq = 3'h0;
reg [2:0] ci0_breq = 3'h0;
reg [2:0] ci1_breq = 3'h0;
reg [2:0] ci2_breq = 3'h0;
reg [2:0] ci3_breq = 3'h0;
reg broadcast_done = 1'b0;
reg ri0_btake = 1'b0;
reg ri1_btake = 1'b0;
reg ri2_btake = 1'b0;
reg ri3_btake = 1'b0;
reg li0_btake = 1'b0;
reg li1_btake = 1'b0;
reg li2_btake = 1'b0;
reg li3_btake = 1'b0;
reg ci0_btake = 1'b0;
reg ci1_btake = 1'b0;
reg ci2_btake = 1'b0;
reg ci3_btake = 1'b0;
reg [4:0] broadcast_delay_counter = 5'h0;
reg start_counting = 1'b0;
reg wait_for_right_response = 1'b0;
reg wait_for_left_take = 1'b0;
reg clear_right_response = 1'b0;
reg [11:0] broadcasting = 12'h0;
reg [2:0] broadcast_response_pending [11:0];
wire left_broadcast_in_progress;
reg wait_for_center_response = 1'b0;
reg clear_center_response = 1'b0;
wire right_broadcast_in_progress;
wire center_broadcast_in_progress;
reg wait_for_center_take = 1'b0;
reg wait_for_right_take = 1'b0;
wire right_response_pending_index;
reg wait_for_side_response = 1'b0;
reg clear_side_response = 1'b0;
reg wait_for_broadcast_finish = 1'b0;
reg wait_for_left_response = 1'b0;
reg [11:0] present_broadcasting = 12'h0;
reg [3:0] number_of_broadcasts_pending = 4'h0;
wire [7:0] save_tag;
reg [11:0] tag_match;
reg [2:0] present_broadcast_response_pending = 3'h0;
reg any_tag_match;
reg [11:0] active_broadcasts = 12'h0;
reg [3:0] broadcast_number;
reg [11:0] save_broadcasting_state [11:0];
reg [3:0] save_broadcast_number = 4'h0;
reg [11:0] found_block = 12'h0;
reg [8:0] broadcast_timeout [11:0];
reg [11:0] broadcast_broadcasting_channels [11:0];
reg [5:0] response_pending_bit [11:0];

reg enable_broadcast_timeout = 1'b0;
reg [3:0] timeout_number = 4'h0;
reg [3:0] save_arbitration_port_number = 4'h0;
reg [2:0] save_breq_final_setting [11:0];
wire broadcast_take;

assign left_broadcast_in_progress = |present_broadcasting[3:0];
assign right_broadcast_in_progress = |present_broadcasting[7:4];
assign center_broadcast_in_progress = |present_broadcasting[11:8];
assign right_response_pending_index = {left_broadcast_in_progress,1'b0};

//2-1 Combinatorial Mux for Left/Right/Center Takes -> broadcast (btake) or normal access (take)
assign li0_take_comb = bcast_stall_switch1 == 1'b1 ? li0_btake : li0_take;
assign li1_take_comb = bcast_stall_switch1 == 1'b1 ? li1_btake : li1_take;
assign li2_take_comb = bcast_stall_switch1 == 1'b1 ? li2_btake : li2_take;
assign li3_take_comb = bcast_stall_switch1 == 1'b1 ? li3_btake : li3_take;
assign ri0_take_comb = bcast_stall_switch1 == 1'b1 ? ri0_btake : ri0_take;
assign ri1_take_comb = bcast_stall_switch1 == 1'b1 ? ri1_btake : ri1_take;
assign ri2_take_comb = bcast_stall_switch1 == 1'b1 ? ri2_btake : ri2_take;
assign ri3_take_comb = bcast_stall_switch1 == 1'b1 ? ri3_btake : ri3_take;
assign ci0_take_comb = bcast_stall_switch1 == 1'b1 ? ci0_btake : ci0_take;
assign ci1_take_comb = bcast_stall_switch1 == 1'b1 ? ci1_btake : ci1_take;
assign ci2_take_comb = bcast_stall_switch1 == 1'b1 ? ci2_btake : ci2_take;
assign ci3_take_comb = bcast_stall_switch1 == 1'b1 ? ci3_btake : ci3_take;

//Check for broadcast requests
assign li0_bcast = li0[93] && li0_ordy && !broadcasting[0];
assign li1_bcast = li1[93] && li1_ordy && !broadcasting[1];
assign li2_bcast = li2[93] && li2_ordy && !broadcasting[2];
assign li3_bcast = li3[93] && li3_ordy && !broadcasting[3];
assign ri0_bcast = ri0[93] && ri0_ordy && !broadcasting[4];
assign ri1_bcast = ri1[93] && ri1_ordy && !broadcasting[5];
assign ri2_bcast = ri2[93] && ri2_ordy && !broadcasting[6];
assign ri3_bcast = ri3[93] && ri3_ordy && !broadcasting[7];
assign ci0_bcast = ci0[93] && ci0_ordy && !broadcasting[8];
assign ci1_bcast = ci1[93] && ci1_ordy && !broadcasting[9];
assign ci2_bcast = ci2[93] && ci2_ordy && !broadcasting[10];
assign ci3_bcast = ci3[93] && ci3_ordy && !broadcasting[11];

//Set High for any broadcast requests
assign bcast_stall_comb = li0_bcast | li1_bcast | li2_bcast | li3_bcast | ri0_bcast | ri1_bcast | ri2_bcast | ri3_bcast | ci0_bcast | ci1_bcast | ci2_bcast | ci3_bcast;

//Check for non broadcast requests
assign li0_normal = !li0[93] && li0_ordy;
assign li1_normal = !li1[93] && li1_ordy;
assign li2_normal = !li2[93] && li2_ordy;
assign li3_normal = !li3[93] && li3_ordy;
assign ri0_normal = !ri0[93] && ri0_ordy;
assign ri1_normal = !ri1[93] && ri1_ordy;
assign ri2_normal = !ri2[93] && ri2_ordy;
assign ri3_normal = !ri3[93] && ri3_ordy;
assign ci0_normal = !ci0[93] && ci0_ordy;
assign ci1_normal = !ci1[93] && ci1_ordy;
assign ci2_normal = !ci2[93] && ci2_ordy;
assign ci3_normal = !ci3[93] && ci3_ordy;
assign bcast_block_comb = li0_normal | li1_normal | li2_normal | li3_normal | ri0_normal | ri1_normal | ri2_normal | ri3_normal | ci0_normal | ci1_normal | ci2_normal | ci3_normal;

//Arbitrate for port broadcast in or raceway broadcast in - 0x0F no broadcast request, fixed arbitration priority
always @(*) //JLSW 
    begin
      arbitration_port_number = 4'hF; //default //JLSW
      if ( li0_bcast == 1 )
        arbitration_port_number = 4'h0; //JLSW
       if ( ri0_bcast == 1 && li0_bcast == 0 )
        arbitration_port_number = 4'h4; //JLSW    
       if ( ci0_bcast == 1 && ri0_bcast == 0 && li0_bcast == 0 )
        arbitration_port_number = 4'h8;  //JLSW                     
    end //end always
    
//Check for response tag matches

//Mux Incoming Tag Bytes
assign save_tag = arbitration_port_number == 4'h0 ? li0[71:64] :
                  arbitration_port_number == 4'h4 ? ri0[71:64] :
                  arbitration_port_number == 4'h8 ? ci0[71:64] :
                                             8'h00;

always @* 
    begin
        tag_match = 12'h000;
		broadcast_number = 4'h0; //JLPL - 11_2_25 -> Do this to removing latching from Synthesis
        if ( bcast_id[0] == save_tag && active_broadcasts[0] )
            begin
                tag_match[0] = 1'b1;
                broadcast_number = 4'h0;
            end //end if bcast_id
        if ( bcast_id[1] == save_tag && active_broadcasts[1] )
            begin
                tag_match[1] = 1'b1;
                broadcast_number = 4'h1;
            end //end if bcast_id 
        if ( bcast_id[2] == save_tag && active_broadcasts[2] )
            begin
                tag_match[2] = 1'b1;
                broadcast_number = 4'h2;
            end //end if bcast_id
        if ( bcast_id[3] == save_tag && active_broadcasts[3] )
            begin
                tag_match[3] = 1'b1;
                broadcast_number = 4'h3;
            end //end if bcast_id
        if ( bcast_id[4] == save_tag && active_broadcasts[4] )
            begin
                tag_match[4] = 1'b1;
                broadcast_number = 4'h4;
            end //end if bcast_id
        if ( bcast_id[5] == save_tag && active_broadcasts[5] )
            begin
                tag_match[5] = 1'b1;
                broadcast_number = 4'h5;
            end //end if bcast_id 
        if ( bcast_id[6] == save_tag && active_broadcasts[6] )
            begin
                tag_match[6] = 1'b1;
                broadcast_number = 4'h6;
            end //end if bcast_id
        if ( bcast_id[7] == save_tag && active_broadcasts[7] )
            begin
                tag_match[7] = 1'b1;
                broadcast_number = 4'h7;
            end //end if bcast_id
        if ( bcast_id[8] == save_tag && active_broadcasts[8] )
            begin
                tag_match[8] = 1'b1;
                broadcast_number = 4'h8;
            end //end if bcast_id
        if ( bcast_id[9] == save_tag && active_broadcasts[9] )
            begin
                tag_match[9] = 1'b1;
                broadcast_number = 4'h9;
            end //end if bcast_id 
        if ( bcast_id[10] == save_tag && active_broadcasts[10] )
            begin
                tag_match[10] = 1'b1;
                broadcast_number = 4'hA;
            end //end if bcast_id
        if ( bcast_id[11] == save_tag && active_broadcasts[11] )
            begin
                tag_match[11] = 1'b1;
                broadcast_number = 4'hB;
            end //end if bcast_id            
            
        if ( tag_match == 12'h0 )
            any_tag_match = 1'b0;
        else
            any_tag_match = 1'b1;
    end //end always

assign broadcast_take =  save_arbitration_port_number == 4'h0 ? li0_take :
                         save_arbitration_port_number == 4'h1 ? li1_take :
                         save_arbitration_port_number == 4'h2 ? li2_take :
                         save_arbitration_port_number == 4'h3 ? li3_take :
                         save_arbitration_port_number == 4'h4 ? ri0_take :
                         save_arbitration_port_number == 4'h5 ? ri1_take :
                         save_arbitration_port_number == 4'h6 ? ri2_take :
                         save_arbitration_port_number == 4'h7 ? ri3_take :
                         save_arbitration_port_number == 4'h8 ? ci0_take :
                         save_arbitration_port_number == 4'h9 ? ci1_take :
                         save_arbitration_port_number == 4'hA ? ci2_take :
                         save_arbitration_port_number == 4'hB ? ci3_take :
                         1'b0;

//Broadcast Transition State Machine
always @ ( posedge clock )
	begin
		if ( reset_clock )
			begin
				Broadcast_State_Register <= 3'b000;
				bcast_stall <= 1'b0;
				bcast_block <= 1'b0;
				bcast_id[0] <= 8'h0;
				bcast_id[1] <= 8'h0;
				bcast_id[2] <= 8'h0;
				bcast_id[3] <= 8'h0;
				bcast_id[4] <= 8'h0;
				bcast_id[5] <= 8'h0;	
				bcast_id[6] <= 8'h0;
				bcast_id[7] <= 8'h0;
				bcast_id[8] <= 8'h0;
				bcast_id[9] <= 8'h0;
				bcast_id[10] <= 8'h0;
				bcast_id[11] <= 8'h0;			
				li0_breq <= 3'h0;
				li1_breq <= 3'h0;
				li2_breq <= 3'h0;
				li3_breq <= 3'h0;
				ri0_breq <= 3'h0;
				ri1_breq <= 3'h0;
				ri2_breq <= 3'h0;
				ri3_breq <= 3'h0;
				ci0_breq <= 3'h0;
				ci1_breq <= 3'h0;
				ci2_breq <= 3'h0;
				ci3_breq <= 3'h0;
				broadcast_done <= 1'b0;
				li0_btake <= 1'b0;
				li1_btake <= 1'b0;
				li2_btake <= 1'b0;
				li3_btake <= 1'b0;
				ri0_btake <= 1'b0;
				ri1_btake <= 1'b0;
				ri2_btake <= 1'b0;
				ri3_btake <= 1'b0;
				ci0_btake <= 1'b0;
				ci1_btake <= 1'b0;
				ci2_btake <= 1'b0;
				ci3_btake <= 1'b0;
				broadcast_delay_counter <= 5'h0;
				start_counting <= 1'b0;
				wait_for_right_response <= 1'b0;
				wait_for_left_take <= 1'b0;
				clear_right_response <= 1'b0;
				broadcasting <= 12'h0;
                broadcast_response_pending[0] <= 3'h0;
                wait_for_center_response <= 1'b0;
                clear_center_response <= 1'b0;
                wait_for_center_take <= 1'b0;
                wait_for_right_take <= 1'b0;
                wait_for_side_response <= 1'b0;
                clear_side_response <= 1'b0;
                wait_for_broadcast_finish <= 1'b0;
                wait_for_left_response <= 1'b0;
                present_broadcasting <= 12'h0;
                number_of_broadcasts_pending <= 4'h0;
                present_broadcast_response_pending <= 3'h0;
                active_broadcasts <= 12'h0;
                save_broadcasting_state[0] <= 12'h0;
                save_broadcast_number <= 4'h0;
                found_block <= 12'h0;
                broadcast_timeout[11] <= 9'h0;
                broadcast_timeout[10] <= 9'h0;
                broadcast_timeout[9] <= 9'h0;
                broadcast_timeout[8] <= 9'h0;
                broadcast_timeout[7] <= 9'h0;
                broadcast_timeout[6] <= 9'h0;
                broadcast_timeout[5] <= 9'h0;
                broadcast_timeout[4] <= 9'h0;
                broadcast_timeout[3] <= 9'h0;
                broadcast_timeout[2] <= 9'h0;
                broadcast_timeout[1] <= 9'h0;
                broadcast_timeout[0] <= 9'h0;

                enable_broadcast_timeout <= 1'b0;
                timeout_number <= 4'h0;
                save_arbitration_port_number <= 4'h0;
			end
		else
		  begin
		  
			case (Broadcast_State_Register)        //Broadcast Re-Transmission and Quadrant Loop Reception State Machine
			
				3'b000	:
					begin	
					   broadcast_delay_counter <= 5'h0;
					   start_counting <= 1'b0;
					   if ( enable_broadcast_timeout ) //Broadcast response timeout enabled
					       begin
				       		   broadcast_timeout[0][7:0] <= broadcast_timeout[0][7:0] + 1;
					           if ( broadcast_timeout[0][7:4] != 4'h0 )
					               begin
					                   if ( timeout_number == number_of_broadcasts_pending || timeout_number == 4'hB )
					                       timeout_number <= 4'h0;
					                   else
					                       begin
					                           if ( broadcast_timeout[0][2:0] == 3'b100 )
					                               timeout_number <= timeout_number + 1;
					                           else
					                               timeout_number <= timeout_number;
					                       end //end else
					                   present_broadcasting <= save_broadcasting_state[timeout_number];
					                   if ( broadcasting[broadcast_broadcasting_channels[timeout_number][3:0]] && ( (broadcasting[broadcast_broadcasting_channels[timeout_number][7:4]] && broadcast_response_pending[timeout_number][response_pending_bit[timeout_number][3:2]]) || (broadcasting[broadcast_broadcasting_channels[timeout_number][11:8]] && broadcast_response_pending[timeout_number][response_pending_bit[timeout_number][5:4]]) ) )					                   
					                       begin
					                           if ( save_broadcasting_state[timeout_number][3:0] != 4'h0 )
					                               begin 
					                                   if ( save_broadcasting_state[timeout_number][0] )
					                                       li0_breq <= 3'h1; //Left Broadcast
					                                   if ( save_broadcasting_state[timeout_number][1] )
					                                       li1_breq <= 3'h1; //Left Broadcast
					                                   if ( save_broadcasting_state[timeout_number][2] )
					                                       li2_breq <= 3'h1; //Left Broadcast					                                       					                                       
					                                   if ( save_broadcasting_state[timeout_number][3] )
					                                       li3_breq <= 3'h1; //Left Broadcast
					                                   wait_for_left_take <= 1'b1;
					                               end //end of if broadcast_present_broadcasting left
					                           if ( save_broadcasting_state[timeout_number][7:4] != 4'h0 )
					                               begin
					                                   if ( save_broadcasting_state[timeout_number][4] ) 
					                                       ri0_breq <= 3'h2; //Right Broadcast
					                                   if ( save_broadcasting_state[timeout_number][5] ) 
					                                       ri1_breq <= 3'h2; //Right Broadcast
					                                   if ( save_broadcasting_state[timeout_number][6] ) 
					                                       ri2_breq <= 3'h2; //Right Broadcast					                                       					                                       					                               
					                                   if ( save_broadcasting_state[timeout_number][7] ) 
					                                       ri3_breq <= 3'h2; //Right Broadcast
					                                   wait_for_right_take <= 1'b1;
					                               end //end of if broadcast_present_broadcasting right	
					                           if ( save_broadcasting_state[timeout_number][11:8] != 4'h0 )
					                               begin
					                                   if ( save_broadcasting_state[timeout_number][8] ) 
					                                       ci0_breq <= 3'h4; //Center Broadcast	
					                                   if ( save_broadcasting_state[timeout_number][9] ) 
					                                       ci1_breq <= 3'h4; //Center Broadcast	
					                                   if ( save_broadcasting_state[timeout_number][10] ) 
					                                       ci2_breq <= 3'h4; //Center Broadcast					                                       				                                       				                               
					                                   if ( save_broadcasting_state[timeout_number][11] ) 
					                                       ci3_breq <= 3'h4; //Center Broadcast
					                                   wait_for_center_take <= 1'b1;
					                               end //end of if broadcast_present_broadcasting center						                               				                               
					                           wait_for_broadcast_finish <= 1'b1;
				                               bcast_stall <= 1'b1;

				                               Broadcast_State_Register <= 3'b101;    
					                       end //end if broadcasting 					                   
					               end //end of if broadcast_timeout[0]					                   
					       end //end of if enable_broadcast
					   else
					       timeout_number <= 4'h0;					       					       					       					       					            
					   					
					   //if ( bcast_stall_comb == 1'b1 ) //JLPL - 9_29_25
					   if ( bcast_stall_comb == 1'b1 && arbitration_port_number != 4'hF ) //JLPL - 9_29_25 - Only respond to the bcast present if a valid arbitration_port_number is present
					       begin
					           bcast_stall <= 1'b1;
					           Broadcast_State_Register <= 3'b001;
					           save_broadcast_number <= broadcast_number;
					           save_arbitration_port_number <= arbitration_port_number;
					           if ( arbitration_port_number != 4'hF )
					               begin
					                   if ( !any_tag_match )  //This is an initial Hub Input Broadcast Request
					                       begin
					                           bcast_id[number_of_broadcasts_pending] <= save_tag; //Store message source address
					                           if ( arbitration_port_number == 4'h0 )
					                               begin
					          				         li0_breq <= 3'h2; //Right Broadcast - Raceway Lane 0
					          				         li1_breq <= 3'h2; //Right Broadcast - Raceway Lane 1
					          				       end	
					                           if ( arbitration_port_number == 4'h4 )
					                               begin
					          				         ri0_breq <= 3'h1; //Left Broadcast //Left Broadcast - Raceway Lane 0
					          				         ri1_breq <= 3'h1; //Left Broadcast //Left Broadcast - Raceway Lane 1
					          				       end
					                           if ( arbitration_port_number == 4'h8 )
					                               begin
					          				         ci0_breq <= 3'h1; //Left Broadcast //To get to Raceway Lane 0
					          				         ci1_breq <= 3'h1; //Left Broadcast //To get to Raceway Lane 1
					          				       end					          				     						          				     				          				     				          				     				          				     
					                           broadcast_done <= 1'b1;
					                           active_broadcasts[number_of_broadcasts_pending] <= 1'b1;
					                           broadcasting <= broadcasting | (12'h001 << arbitration_port_number);
					                           present_broadcasting <= (12'h001 << arbitration_port_number);
					                           save_broadcasting_state[number_of_broadcasts_pending] <= (12'h001 << arbitration_port_number);
					                           if ( arbitration_port_number[3:2] == 2'b00 )
					                               begin
                                                        broadcast_response_pending[number_of_broadcasts_pending] <= 3'b110;
                                                        present_broadcast_response_pending <= 3'b110;
                                                        save_breq_final_setting[number_of_broadcasts_pending] <= 3'b001;
                                                   end
					                           if ( arbitration_port_number[3:2] == 2'b01 )
					                               begin
                                                        broadcast_response_pending[number_of_broadcasts_pending] <= 3'b101;
                                                        present_broadcast_response_pending <= 3'b101;
                                                        save_breq_final_setting[number_of_broadcasts_pending] <= 3'b010;
                                                        Broadcast_State_Register <= 3'b100;
                                                   end
					                           if ( arbitration_port_number[3:2] == 2'b10 ) //Check Center
					                               begin
                                                        broadcast_response_pending[number_of_broadcasts_pending] <= 3'b011;
                                                        present_broadcast_response_pending <= 3'b011;
                                                        save_breq_final_setting[number_of_broadcasts_pending] <= 3'b100;
                                                        Broadcast_State_Register <= 3'b011;
                                                   end                                                                      
                                               broadcast_timeout[number_of_broadcasts_pending] <= 9'h100; //Start Timeout Timer
                                               broadcast_broadcasting_channels [number_of_broadcasts_pending] <= {4'h8,4'h4,4'h0};
                                               response_pending_bit[number_of_broadcasts_pending] <= {2'h2,2'h1,2'h0};
                                               enable_broadcast_timeout <= 1'b0;
                                               number_of_broadcasts_pending <= number_of_broadcasts_pending + 1;
					                       end //end of if !right_broadcast_in_progress
					                   else //else if any_match ( response to broadcast request )
					                       begin
					                           wait_for_side_response <= 1'b1;
					                           if ( arbitration_port_number[3:2] == 2'b00 ) //Check Left
					                               begin
					                                   wait_for_left_response <= 1'b1;
					                                   broadcast_response_pending[broadcast_number] <= broadcast_response_pending[broadcast_number] & 3'h6; //Clear Left Side Response
					                                   present_broadcast_response_pending <= broadcast_response_pending[broadcast_number] & 3'h6;
					                               end 
					                           if ( arbitration_port_number[3:2] == 2'b01 ) //Check Right
					                               begin
					                                   wait_for_right_response <= 1'b1;
					                                   broadcast_response_pending[broadcast_number] <= broadcast_response_pending[broadcast_number] & 3'h5; //Clear Right Side Response
					                                   present_broadcast_response_pending <= broadcast_response_pending[broadcast_number] & 3'h5;
					                               end
					                           if ( arbitration_port_number[3:2] == 2'b10 ) //Check Center
					                               begin
					                                   wait_for_center_response <= 1'b1;
					                                   broadcast_response_pending[broadcast_number] <= broadcast_response_pending[broadcast_number] & 3'h3; //Clear Center Side Response
					                                   present_broadcast_response_pending <= broadcast_response_pending[broadcast_number] & 3'h3;
					                               end					                               					                               
					                           present_broadcasting <= save_broadcasting_state[broadcast_number];
					                           broadcast_done <= 1'b1;
					                           Broadcast_State_Register <= 3'b001;				          
					                       end //end of else !right_broadcast_in_progress                                              
					               end //end of if left, right or center inputs
				           end //end of if bcast_stall_comb
					
					   if ( bcast_stall == 1'b0 && bcast_stall_comb == 1'b0 && bcast_block_comb == 1'b1 )
					       bcast_block <= 1'b1;
					   else
					       bcast_block <= 1'b0;
					end //end broadcast idle mode

				3'b001	:	
				    begin
				       if ( wait_for_broadcast_finish )
				          begin
				            if ( broadcast_take )
				                begin
				                    start_counting <= 1'b1;
				                    wait_for_broadcast_finish <= 1'b0;
				                    li0_breq <= 3'h0;
				                    li1_breq <= 3'h0;
				                    li2_breq <= 3'h0;
				                    li3_breq <= 3'h0;
				                    ri0_breq <= 3'h0;
				                    ri1_breq <= 3'h0;
				                    ri2_breq <= 3'h0;
				                    ri3_breq <= 3'h0;
				                    ci0_breq <= 3'h0;
				                    ci1_breq <= 3'h0;
				                    ci2_breq <= 3'h0;
				                    ci3_breq <= 3'h0;	
				                    broadcasting <= 12'h0; //JLPL - 9_29_25 - Hold off any pending broadcasts until this broadcast is finished						
				                end //end of if broadcast_take        				                
				          end //end of if wait_for_broadcast_finish

				       if ( !wait_for_broadcast_finish )
				          begin                   
				            if ( li0_breq == 4'h2 && li0_take == 1'b1 ) //Right Broadcast - Raceway Lane 0 Accepted
				                begin
				                    li0_breq <= 3'h4; //Go to Center Broadcast - Raceway Lane 0
				                    li1_breq <= 3'h4; //Go to Center Broadcast - Raceway Lane 1
				                end
				            if ( li0_breq == 4'h4 && li0_take == 1'b1 ) //Center Broadcast
				                begin
				                   bcast_stall <= 1'b0;
					               broadcast_done <= 1'b0;
					               Broadcast_State_Register <= 3'b010;
				                   li0_breq <= 3'h0;
				                   li0_btake <= 1'b1;
				                   li1_breq <= 3'h0;
				                   li1_btake <= 1'b1;
				                   //broadcasting <= broadcasting & ~present_broadcasting; //JLPL - 9_29_25 - Hold off any pending broadcasts until this broadcast is finished			                
				                end 
                          end   //end of if !wait_for_broadcast_finish				                
				       
				       if ( clear_side_response == 1'b1 )
				        begin
				            clear_side_response <= 1'b0;
				            li0_btake <= 1'b0; 
				            li1_btake <= 1'b0; 
				            li2_btake <= 1'b0; 
				            li3_btake <= 1'b0; 
				            ri0_btake <= 1'b0; 
				            ri1_btake <= 1'b0; 
				            ri2_btake <= 1'b0; 
				            ri3_btake <= 1'b0;
				            ci0_btake <= 1'b0; 
				            ci1_btake <= 1'b0; 
				            ci2_btake <= 1'b0; 
				            ci3_btake <= 1'b0; 			            
				        end //end if clear_side_response
				            
				       if ( wait_for_side_response == 1'b1 )
				            begin
				                wait_for_side_response <= 1'b0;
				                if ( wait_for_right_response == 1'b1 && present_broadcast_response_pending != 3'b000 )
				                    begin
				                        wait_for_right_response <= 1'b0;				                
				                        if ( ri0_bcast == 1'b1 && save_arbitration_port_number == 4'h4 )
				                            begin
				                                ri0_btake <= 1'b1;
				                                ri1_btake <= 1'b1;
				                            end	
				                    end //end of if wait_for_right_response
				                if ( wait_for_center_response == 1'b1 && present_broadcast_response_pending != 3'b000 )
				                    begin
				                        wait_for_center_response <= 1'b0;			                
				                        if ( ci0_bcast == 1'b1 && save_arbitration_port_number == 4'h8 )
											begin
												ci0_btake <= 1'b1;
												ci1_btake <= 1'b1;
											end	
				                    end //end of if wait_for_center_response	
				                if ( wait_for_left_response == 1'b1 && present_broadcast_response_pending != 3'b000 )
				                    begin
				                        wait_for_left_response <= 1'b0;				                
				                        if ( li0_bcast == 1'b1 && save_arbitration_port_number == 4'h0 )
				                            begin
				                                li0_btake <= 1'b1;
				                                li1_btake <= 1'b1;
				                            end
				                    end //end of if wait_for_left_response
				                    			                    			                    				                    
				                if ( present_broadcast_response_pending == 3'b000 ) //Check if Any Response Still Pending and no broadcast deadlock residue pending
				                    begin
				                        active_broadcasts[save_broadcast_number] <= 1'b0;
				                        present_broadcasting[save_broadcast_number] <= 1'b0;
				                        broadcast_timeout[save_broadcast_number] <= 9'h0;	
	                         			if ( save_arbitration_port_number == 4'h0 )
	                         			   begin
				                                li0_breq <= save_breq_final_setting[save_broadcast_number]; //Left Broadcast Raceway Lane 0
				                                li1_breq <= save_breq_final_setting[save_broadcast_number]; //Left Broadcast Raceway Lane 1
				                            end
				                        if ( save_arbitration_port_number == 4'h4 )
				                           begin
				                                ri0_breq <= save_breq_final_setting[save_broadcast_number]; //Right Broadcast Raceway Lane 0
				                                ri1_breq <= save_breq_final_setting[save_broadcast_number]; //Right Broadcast Raceway Lane 1
				                           end
				                        if ( save_arbitration_port_number == 4'h8 )
				                            begin
				                                ci0_breq <= save_breq_final_setting[save_broadcast_number]; //Center Broadcast - Raceway Lane 0
				                                ci1_breq <= save_breq_final_setting[save_broadcast_number]; //Center Broadcast - Raceway Lane 1				                                
				                            end
				                        wait_for_broadcast_finish <= 1'b1;				                            
				                    end //end of if broadcast ...
				                else
				                    begin
				                        bcast_stall <= 1'b0;
					                    broadcast_done <= 1'b0;
					                    Broadcast_State_Register <= 3'b010;				                            
				                    end //end of else broadcast ...
			                end //end of wait_for_side_response
			                			                
				       
				       if ( start_counting == 1'b1 ) 
				            broadcast_delay_counter <= broadcast_delay_counter + 1;			         
				       				    
				       if ( broadcast_done == 1'b1 && broadcast_delay_counter[1] == 1'b1 )
				        begin
				           if ( save_arbitration_port_number == 4'h0 )
				            begin
				                li0_btake <= 1'b1;
				                li1_btake <= 1'b1; //Need to take both Raceway Lane 0 and Raceway Lane 1
				            end
				           if ( save_arbitration_port_number == 4'h4 )
				            begin
				                ri0_btake <= 1'b1;
				                ri1_btake <= 1'b1;
				            end
				           if ( save_arbitration_port_number == 4'h8 )
							begin
				                ci0_btake <= 1'b1;
								ci1_btake <= 1'b1;
							end
					       bcast_stall <= 1'b0;
					       broadcast_done <= 1'b0;
					       Broadcast_State_Register <= 3'b010;
					    end				    
				    end //end broadcast stall mode	
				    
				3'b010	:	
				    begin
				       if ( broadcasting == 12'h000 )
				        begin
				            if ( active_broadcasts == 12'h000 )
				                number_of_broadcasts_pending <= 4'h0;
				            broadcast_timeout[0] <= 9'h0;
				            enable_broadcast_timeout <= 1'b0;
				            timeout_number <= 4'h0;
				        end   
				       li0_btake <= 1'b0; 
				       li1_btake <= 1'b0; 
				       li2_btake <= 1'b0; 
				       li3_btake <= 1'b0; 
				       ri0_btake <= 1'b0; 
				       ri1_btake <= 1'b0; 
				       ri2_btake <= 1'b0; 
				       ri3_btake <= 1'b0; 
				       ci0_btake <= 1'b0; 
				       ci1_btake <= 1'b0; 
				       ci2_btake <= 1'b0; 
				       ci3_btake <= 1'b0;
					   Broadcast_State_Register <= 3'b000;			    
				    end //end of delay until left side input is cleared away
				    
				3'b011	:	
				    begin

				       if ( ci0_breq == 4'h1 && ci0_take == 1'b1 )
				        begin
				            ci0_breq <= 3'h2; //Go from Left Broadcast to Right Broadcast Request - Raceway Lane 0
				            ci1_breq <= 3'h2; //Go from Left Broadcast to Right Broadcast Request - Raceway Lane 1
				        end
				       if ( ci0_breq == 4'h2 && ci0_take == 1'b1 )
				            begin
				                start_counting <= 1'b1;
				                ci0_breq <= 3'h0;
				                ci0_btake <= 1'b1;
				                ci1_breq <= 3'h0;
				                ci1_btake <= 1'b1;				                
				                broadcasting <= broadcasting & ~present_broadcasting;				                
				            end
				       if ( ci0_breq == 4'h4 && ci0_take == 1'b1 )
				            begin
				                start_counting <= 1'b1;
				                ci0_breq <= 3'h0;
				            end
				            				            				       
				       if ( start_counting == 1'b1 ) 
				            begin
				                ci0_btake <= 1'b0;
				                ci1_btake <= 1'b0;
				                ci2_btake <= 1'b0;
				                ci3_btake <= 1'b0;
				                broadcast_delay_counter <= broadcast_delay_counter + 1;
				            end //end if start_counting			         
				       				    
				       if ( broadcast_done == 1'b1 && broadcast_delay_counter[1] == 1'b1 )
				        begin
					       bcast_stall <= 1'b0;
					       broadcast_done <= 1'b0;
					       Broadcast_State_Register <= 3'b010;
					    end	
				    end //end of delay until right side input is cleared away
				    
				3'b100	:	
				    begin

				       if ( ri0_breq == 4'h1 && ri0_take == 1'b1 ) //Left Broadcast - Raceway Lane 0 Accepted
				        begin
				            ri0_breq <= 3'h4; //Go to Center
				            ri1_breq <= 3'h4;
				        end
				       if ( ri0_breq == 4'h4 && ri0_take == 1'b1 )
				            begin
				                start_counting <= 1'b1;
				                ri0_breq <= 3'h0;
				                ri0_btake <= 1'b1;
				                ri1_breq <= 3'h0;
				                ri1_btake <= 1'b1;
				                broadcasting <= broadcasting & ~present_broadcasting;				                
				            end
				       
				       if ( start_counting == 1'b1 ) 
				            begin
				                ri0_btake <= 1'b0;
				                ri1_btake <= 1'b0;
				                ri2_btake <= 1'b0;
				                ri3_btake <= 1'b0;
				                broadcast_delay_counter <= broadcast_delay_counter + 1;	
				            end	// end start_counting		         
				       				    
				       if ( broadcast_done == 1'b1 && broadcast_delay_counter[1] == 1'b1 )
				        begin
					       bcast_stall <= 1'b0;
					       broadcast_done <= 1'b0;
					       Broadcast_State_Register <= 3'b010;
					    end				    
				    end //end of right side clear					    			

				3'b101	:	
				    begin
				        			        				        				        				        				    
				       if ( wait_for_broadcast_finish )
				          begin
				            if ( wait_for_left_take )
				                begin
				                    if ( (present_broadcasting[0] && li0_take) )
				                        begin
					                        bcast_stall <= 1'b0;
					                        broadcast_done <= 1'b0;
					                        Broadcast_State_Register <= 3'b010;
				                            wait_for_broadcast_finish <= 1'b0;
				                            wait_for_left_take <= 1'b0;
				                            li0_breq <= 3'h0;
				                            li1_breq <= 3'h0;
				                            li2_breq <= 3'h0;
				                            li3_breq <= 3'h0;
				                            if ( present_broadcasting[0] == 1'b1 )
				                                begin 
				                                    broadcasting[0] <= 1'b0;
				                                    present_broadcasting[0] <= 1'b0;
				                                    li0_btake <= 1'b1;
				                                    li1_btake <= 1'b1;
				                                end //end of if present_broadcasting[0]				                            
				                        end //end of if broadcasting ...				                    
				                end //end of if wait_for_left_take
				                
				            if ( wait_for_center_take )
				                begin
				                    if ( (present_broadcasting[8] && ci0_take) || (present_broadcasting[9] && ci1_take) || (present_broadcasting[10] && ci2_take) || (present_broadcasting[11] && ci3_take) )
				                        begin
					                        bcast_stall <= 1'b0;
					                        broadcast_done <= 1'b0;
					                        Broadcast_State_Register <= 3'b010;
				                            wait_for_broadcast_finish <= 1'b0;
				                            wait_for_center_take <= 1'b0;
				                            ci0_breq <= 3'h0;
				                            ci1_breq <= 3'h0;
				                            ci2_breq <= 3'h0;
				                            ci3_breq <= 3'h0;
				                            if ( present_broadcasting[8] == 1'b1 )
				                                begin 
				                                    broadcasting[8] <= 1'b0;
				                                    present_broadcasting[8] <= 1'b0;
				                                    ci0_btake <= 1'b1;
													ci1_btake <= 1'b1;
				                                end //end of if present_broadcasting[8]
				                            if ( present_broadcasting[9] == 1'b1 ) 
				                                begin 
				                                    broadcasting[9] <= 1'b0;
				                                    present_broadcasting[9] <= 1'b0; 
				                                    ci1_btake <= 1'b1;
				                                end //end of if present_broadcasting[9]
				                            if ( present_broadcasting[10] == 1'b1 ) 
				                                begin 
				                                    broadcasting[10] <= 1'b0;
				                                    present_broadcasting[10] <= 1'b0;
				                                    ci2_btake <= 1'b1;
				                                end //end of if present_broadcasting[10]
				                            if ( present_broadcasting[11] == 1'b1 )
				                                begin 
				                                    broadcasting[11] <= 1'b0; 
				                                    present_broadcasting[11] <= 1'b0;
				                                    ci3_btake <= 1'b1;
				                                end //end of if present_broadcasting[11]				                            
				                        end //end of if broadcasting ...				                    
				                end //end of if wait_for_center_take
				                
				            if ( wait_for_right_take )
				                begin
				                    if ( (present_broadcasting[4] && ri0_take) || (present_broadcasting[5] && ri1_take) || (present_broadcasting[6] && ri2_take) || (present_broadcasting[7] && ri3_take) )
				                        begin
					                        bcast_stall <= 1'b0;
					                        broadcast_done <= 1'b0;
					                        Broadcast_State_Register <= 3'b010;
				                            wait_for_broadcast_finish <= 1'b0;
				                            wait_for_right_take <= 1'b0;
				                            ri0_breq <= 3'h0;
				                            ri1_breq <= 3'h0;
				                            ri2_breq <= 3'h0;
				                            ri3_breq <= 3'h0;
				                            if ( present_broadcasting[4] == 1'b1 )
				                                begin 
				                                    broadcasting[4] <= 1'b0;
				                                    present_broadcasting[4] <= 1'b0;
				                                    ri0_btake <= 1'b1;
				                                end //end of if present_broadcasting[4]
				                            if ( present_broadcasting[5] == 1'b1 ) 
				                                begin 
				                                    broadcasting[5] <= 1'b0;
				                                    present_broadcasting[5] <= 1'b0; 
				                                    ri1_btake <= 1'b1;
				                                end //end of if present_broadcasting[5]
				                            if ( present_broadcasting[6] == 1'b1 ) 
				                                begin 
				                                    broadcasting[6] <= 1'b0;
				                                    present_broadcasting[6] <= 1'b0;
				                                    ri2_btake <= 1'b1;
				                                end //end of if present_broadcasting[6]
				                            if ( present_broadcasting[7] == 1'b1 )
				                                begin 
				                                    broadcasting[7] <= 1'b0; 
				                                    present_broadcasting[7] <= 1'b0;
				                                    ri3_btake <= 1'b1;
				                                end //end of if present_broadcasting[7]				                            
				                        end //end of if broadcasting ...				                    
				                end //end of if wait_for_right_take				                				                
				          end //end of if wait_for_broadcast_finish				    
				    			            			    
				    end //end of deadlock broadcast request clear

				default :
					begin
						Broadcast_State_Register <= 3'b000;
				        bcast_stall <= 1'b0;
				        bcast_block <= 1'b0;
				        li0_breq <= 3'h0;
				        li1_breq <= 3'h0;
				        li2_breq <= 3'h0;
				        li3_breq <= 3'h0;
				        ri0_breq <= 3'h0;
				        ri1_breq <= 3'h0;
				        ri2_breq <= 3'h0;
				        ri3_breq <= 3'h0;
				        ci0_breq <= 3'h0;
				        ci1_breq <= 3'h0;
				        ci2_breq <= 3'h0;
				        ci3_breq <= 3'h0;
				        broadcast_done <= 1'b0;
				        li0_btake <= 1'b0;
				        li1_btake <= 1'b0;
				        li2_btake <= 1'b0;
				        li3_btake <= 1'b0;
				        ri0_btake <= 1'b0;
				        ri1_btake <= 1'b0;
				        ri2_btake <= 1'b0;
				        ri3_btake <= 1'b0;
				        broadcast_delay_counter <= 5'h0;
				        start_counting <= 1'b0;
				        wait_for_right_response <= 1'b0;
				        wait_for_left_take <= 1'b0;
				        clear_right_response <= 1'b0;
				        broadcasting <= 12'h0;
                        broadcast_response_pending[0] <= 3'h0;
                        wait_for_center_response <= 1'b0;
                        clear_center_response <= 1'b0;
                        wait_for_center_take <= 1'b0;
                        wait_for_right_take <= 1'b0;
                        wait_for_side_response <= 1'b0;
                        clear_side_response <= 1'b0;
                        wait_for_broadcast_finish <= 1'b0;
                        wait_for_left_response <= 1'b0;
                        present_broadcasting <= 12'h0;
				        bcast_id[0] <= 8'h0;
				        bcast_id[1] <= 8'h0;
				        bcast_id[2] <= 8'h0;
				        bcast_id[3] <= 8'h0;
				        bcast_id[4] <= 8'h0;
				        bcast_id[5] <= 8'h0;	
				        bcast_id[6] <= 8'h0;
				        bcast_id[7] <= 8'h0;
				        bcast_id[8] <= 8'h0;
				        bcast_id[9] <= 8'h0;
				        bcast_id[10] <= 8'h0;
				        bcast_id[11] <= 8'h0;				        			        
				        number_of_broadcasts_pending <= 4'h0;
				        present_broadcast_response_pending <= 3'h0;
				        active_broadcasts <= 12'h0;
				        save_broadcasting_state[0] <= 12'h0;
				        save_broadcast_number <= 4'h0;
				        found_block <= 12'h0; 
                        broadcast_timeout[11] <= 9'h0;
                        broadcast_timeout[10] <= 9'h0;
                        broadcast_timeout[9] <= 9'h0;
                        broadcast_timeout[8] <= 9'h0;
                        broadcast_timeout[7] <= 9'h0;
                        broadcast_timeout[6] <= 9'h0;
                        broadcast_timeout[5] <= 9'h0;
                        broadcast_timeout[4] <= 9'h0;
                        broadcast_timeout[3] <= 9'h0;
                        broadcast_timeout[2] <= 9'h0;
                        broadcast_timeout[1] <= 9'h0;
                        broadcast_timeout[0] <= 9'h0;

                        enable_broadcast_timeout <= 1'b0;
                        timeout_number <= 4'h0;
                        save_arbitration_port_number <= 4'h0;				                              				        
					end //end default
					
			endcase //end case
          end //end else reset
	end //end always


// LANE-TO-LANE =================================================================================================================
// Decode Destinations and create requests
// 12 arbiters each with 3 requests
wire  [2:0] li0_req_dec  =  li0_normal ? ((li0[79]==ID) ? ( li0[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // If we match North Use Broadcast Qualified ordy
            ri0_req_dec  =  ri0_normal ? ((ri0[79]==ID) ? ( ri0[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // South we check for    - Use Broadcast Qualified ordy 
            ci0_req_dec  =  ci0_normal ? ((ci0[79]==ID) ? ( ci0[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // East and West and     - Use Broadcast Qualified ordy  
                                                                                                                // steer East\West         
            li1_req_dec  =  li1_normal ? ((li1[79]==ID) ? ( li1[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // else we send to the   - Use Broadcast Qualified ordy 
            ri1_req_dec  =  ri1_normal ? ((ri1[79]==ID) ? ( ri1[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // column to the         - Use Broadcast Qualified ordy
            ci1_req_dec  =  ci1_normal ? ((ci1[79]==ID) ? ( ci1[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // upper/lower half!     - Use Broadcast Qualified ordy
                                                                                                                //
            li2_req_dec  =  li2_normal ? ((li2[79]==ID) ? ( li2[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // 001: go left          - Use Broadcast Qualified ordy         
            ri2_req_dec  =  ri2_normal ? ((ri2[79]==ID) ? ( ri2[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // 010: go right         - Use Broadcast Qualified ordy
            ci2_req_dec  =  ci2_normal ? ((ci2[79]==ID) ? ( ci2[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // 100: go up/down       - Use Broadcast Qualified ordy

            li3_req_dec  =  li3_normal ? ((li3[79]==ID) ? ( li3[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // Use Broadcast Qualified ordy                        
            ri3_req_dec  =  ri3_normal ? ((ri3[79]==ID) ? ( ri3[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000,    // Use Broadcast Qualified ordy                        
            ci3_req_dec  =  ci3_normal ? ((ci3[79]==ID) ? ( ci3[75] ?  3'b010 : 3'b001 ) : 3'b100) : 3'b000;    // Use Broadcast Qualified ordy                       

wire [2:0] li0_req;
wire [2:0] ri0_req;
wire [2:0] ci0_req;
wire [2:0] li1_req;
wire [2:0] ri1_req;
wire [2:0] ci1_req;
wire [2:0] li2_req;
wire [2:0] ri2_req;
wire [2:0] ci2_req;
wire [2:0] li3_req;
wire [2:0] ri3_req;
wire [2:0] ci3_req;
assign li0_req = bcast_stall_switch == 1'b1 ? li0_breq : li0_req_dec; 
assign ri0_req = bcast_stall_switch == 1'b1 ? ri0_breq : ri0_req_dec;
assign ci0_req = bcast_stall_switch == 1'b1 ? ci0_breq : ci0_req_dec;
assign li1_req = bcast_stall_switch == 1'b1 ? li1_breq : li1_req_dec; 
assign ri1_req = bcast_stall_switch == 1'b1 ? ri1_breq : ri1_req_dec;
assign ci1_req = bcast_stall_switch == 1'b1 ? ci1_breq : ci1_req_dec;
assign li2_req = bcast_stall_switch == 1'b1 ? li2_breq : li2_req_dec; 
assign ri2_req = bcast_stall_switch == 1'b1 ? ri2_breq : ri2_req_dec;
assign ci2_req = bcast_stall_switch == 1'b1 ? ci2_breq : ci2_req_dec;
assign li3_req = bcast_stall_switch == 1'b1 ? li3_breq : li3_req_dec; 
assign ri3_req = bcast_stall_switch == 1'b1 ? ri3_breq : ri3_req_dec;
assign ci3_req = bcast_stall_switch == 1'b1 ? ci3_breq : ci3_req_dec;


wire  [2:0] lo0_win,ro0_win,co0_win,lo1_win,ro1_win,co1_win,lo2_win,ro2_win,co2_win,lo3_win,ro3_win,co3_win;

A2_arb3 i_arb1  (.grant(lo0_win[2:0]),.request({li0_req[0],ri0_req[0],ci0_req[0]}),.advance(L_QO_rd[0]),`CKRS);
A2_arb3 i_arb2  (.grant(ro0_win[2:0]),.request({li0_req[1],ri0_req[1],ci0_req[1]}),.advance(R_QO_rd[0]),`CKRS);
A2_arb3 i_arb3  (.grant(co0_win[2:0]),.request({li0_req[2],ri0_req[2],ci0_req[2]}),.advance(Co0_fb),`CKRS);

A2_arb3 i_arb4  (.grant(lo1_win[2:0]),.request({li1_req[0],ri1_req[0],ci1_req[0]}),.advance(L_QO_rd[1]),`CKRS);
A2_arb3 i_arb5  (.grant(ro1_win[2:0]),.request({li1_req[1],ri1_req[1],ci1_req[1]}),.advance(R_QO_rd[1]),`CKRS);
A2_arb3 i_arb6  (.grant(co1_win[2:0]),.request({li1_req[2],ri1_req[2],ci1_req[2]}),.advance(Co1_fb),`CKRS);

A2_arb3 i_arb7  (.grant(lo2_win[2:0]),.request({li2_req[0],ri2_req[0],ci2_req[0]}),.advance(L_QO_rd[2]),`CKRS);
A2_arb3 i_arb8  (.grant(ro2_win[2:0]),.request({li2_req[1],ri2_req[1],ci2_req[1]}),.advance(R_QO_rd[2]),`CKRS);
A2_arb3 i_arb9  (.grant(co2_win[2:0]),.request({li2_req[2],ri2_req[2],ci2_req[2]}),.advance(Co2_fb),`CKRS);

A2_arb3 i_arb10 (.grant(lo3_win[2:0]),.request({li3_req[0],ri3_req[0],ci3_req[0]}),.advance(L_QO_rd[3]),`CKRS);
A2_arb3 i_arb11 (.grant(ro3_win[2:0]),.request({li3_req[1],ri3_req[1],ci3_req[1]}),.advance(R_QO_rd[3]),`CKRS);
A2_arb3 i_arb12 (.grant(co3_win[2:0]),.request({li3_req[2],ri3_req[2],ci3_req[2]}),.advance(Co3_fb),`CKRS);

// push to outputs
wire        lo0push     =  |lo0_win[2:0],
            ro0push     =  |ro0_win[2:0],
            co0push     =  |co0_win[2:0],

            lo1push     =  |lo1_win[2:0],
            ro1push     =  |ro1_win[2:0],
            co1push     =  |co1_win[2:0],

            lo2push     =  |lo2_win[2:0],
            ro2push     =  |ro2_win[2:0],
            co2push     =  |co2_win[2:0],

            lo3push     =  |lo3_win[2:0],
            ro3push     =  |ro3_win[2:0],
            co3push     =  |co3_win[2:0];

// pop (take) from inputs
assign      li0_take    =  L_QO_rd[0] & lo0_win[2] | R_QO_rd[0] & ro0_win[2] | Co0_fb & co0_win[2],
            ri0_take    =  L_QO_rd[0] & lo0_win[1] | R_QO_rd[0] & ro0_win[1] | Co0_fb & co0_win[1],
            ci0_take    =  L_QO_rd[0] & lo0_win[0] | R_QO_rd[0] & ro0_win[0] | Co0_fb & co0_win[0],

            li1_take    =  L_QO_rd[1] & lo1_win[2] | R_QO_rd[1] & ro1_win[2] | Co1_fb & co1_win[2],
            ri1_take    =  L_QO_rd[1] & lo1_win[1] | R_QO_rd[1] & ro1_win[1] | Co1_fb & co1_win[1],
            ci1_take    =  L_QO_rd[1] & lo1_win[0] | R_QO_rd[1] & ro1_win[0] | Co1_fb & co1_win[0],

            li2_take    =  L_QO_rd[2] & lo2_win[2] | R_QO_rd[2] & ro2_win[2] | Co2_fb & co2_win[2],
            ri2_take    =  L_QO_rd[2] & lo2_win[1] | R_QO_rd[2] & ro2_win[1] | Co2_fb & co2_win[1],
            ci2_take    =  L_QO_rd[2] & lo2_win[0] | R_QO_rd[2] & ro2_win[0] | Co2_fb & co2_win[0],

            li3_take    =  L_QO_rd[3] & lo3_win[2] | R_QO_rd[3] & ro3_win[2] | Co3_fb & co3_win[2],
            ri3_take    =  L_QO_rd[3] & lo3_win[1] | R_QO_rd[3] & ro3_win[1] | Co3_fb & co3_win[1],
            ci3_take    =  L_QO_rd[3] & lo3_win[0] | R_QO_rd[3] & ro3_win[0] | Co3_fb & co3_win[0];

// Outputs ======================================================================================================================

wire [96:0] Lo0,Lo1,Lo2,Lo3;
wire [96:0] Ro0,Ro1,Ro2,Ro3;

assign      Lo0[96:0]   =  { lo0push,li0[95:0], lo0push,ri0[95:0], lo0push,ci0[95:0] } >> (97 * lo0_win[2:1]), // Change Push
            Ro0[96:0]   =  { ro0push,li0[95:0], ro0push,ri0[95:0], ro0push,ci0[95:0] } >> (97 * ro0_win[2:1]), // Change Push
            Co0[96:0]   =  { co0push,li0[95:0], co0push,ri0[95:0], co0push,ci0[95:0] } >> (97 * co0_win[2:1]), // Change Push

            Lo1[96:0]   =  { lo1push,li1[95:0], lo1push,ri1[95:0], lo1push,ci1[95:0] } >> (97 * lo1_win[2:1]), // Change Push
            Ro1[96:0]   =  { ro1push,li1[95:0], ro1push,ri1[95:0], ro1push,ci1[95:0] } >> (97 * ro1_win[2:1]), // Change Push 
            Co1[96:0]   =  { co1push,li1[95:0], co1push,ri1[95:0], co1push,ci1[95:0] } >> (97 * co1_win[2:1]), // Change Push

            Lo2[96:0]   =  { lo2push,li2[95:0], lo2push,ri2[95:0], lo2push,ci2[95:0] } >> (97 * lo2_win[2:1]), // Change Push
            Ro2[96:0]   =  { ro2push,li2[95:0], ro2push,ri2[95:0], ro2push,ci2[95:0] } >> (97 * ro2_win[2:1]), // Change Push
            Co2[96:0]   =  { co2push,li2[95:0], co2push,ri2[95:0], co2push,ci2[95:0] } >> (97 * co2_win[2:1]), // Change Push

            Lo3[96:0]   =  { lo3push,li3[95:0], lo3push,ri3[95:0], lo3push,ci3[95:0] } >> (97 * lo3_win[2:1]), // Change Push
            Ro3[96:0]   =  { ro3push,li3[95:0], ro3push,ri3[95:0], ro3push,ci3[95:0] } >> (97 * ro3_win[2:1]), // Change Push
            Co3[96:0]   =  { co3push,li3[95:0], co3push,ri3[95:0], co3push,ci3[95:0] } >> (97 * co3_win[2:1]); // Change Push


assign L_QO_or[0] = Lo0[96];
assign L_QO_or[1] = Lo1[96];
assign L_QO_or[2] = Lo2[96];
assign L_QO_or[3] = Lo3[96];
assign L_QO[95:0] = Lo0[95:0];
assign L_QO[191:96] = Lo1[95:0];
assign L_QO[287:192] = Lo2[95:0];
assign L_QO[383:288] = Lo3[95:0];

assign R_QO_or[0] = Ro0[96];
assign R_QO_or[1] = Ro1[96];
assign R_QO_or[2] = Ro2[96];
assign R_QO_or[3] = Ro3[96];
assign R_QO[95:0] = Ro0[95:0];
assign R_QO[191:96] = Ro1[95:0];
assign R_QO[287:192] = Ro2[95:0];
assign R_QO[383:288] = Ro3[95:0];

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
`ifdef   verify_racewayhub

module t;

reg  [96:0] Li0,      Li1,      Li2,      Li3;
reg  [96:0] Ri0,      Ri1,      Ri2,      Ri3;
reg  [96:0] Ci0,      Ci1,      Ci2,      Ci3;
reg         Li0_push, Li1_push, Li2_push, Li3_push;
reg         Lo0_irdy, Lo1_irdy, Lo2_irdy, Lo3_irdy;
reg         Ro0_irdy, Ro1_irdy, Ro2_irdy, Ro3_irdy;
reg         Co0_irdy, Co1_irdy, Co2_irdy, Co3_irdy;
reg         Ri0_push, Ri1_push, Ri2_push, Ri3_push;
reg         Ci0_push, Ci1_push, Ci2_push, Ci3_push;
reg         ID;     // 0 = This module is the NORTH Hub, 1 = This module is the SOUTH hub
reg         reset;
reg         clock;

wire [96:0] Lo0,      Lo1,      Lo2,      Lo3;
wire [96:0] Ro0,      Ro1,      Ro2,      Ro3;
wire [96:0] Co0,      Co1,      Co2,      Co3;
wire        Lo0_push, Lo1_push, Lo2_push, Lo3_push;
wire        Ro0_push, Ro1_push, Ro2_push, Ro3_push;
wire        Co0_push, Co1_push, Co2_push, Co3_push;
wire        Li0_irdy, Li1_irdy, Li2_irdy, Li3_irdy;
wire        Ri0_irdy, Ri1_irdy, Ri2_irdy, Ri3_irdy;
wire        Ci0_irdy, Ci1_irdy, Ci2_irdy, Ci3_irdy;

RacewayHub mut (
   .Lo0   (Lo0),    .Lo1   (Lo1),    .Lo2   (Lo2),    .Lo3   (Lo3),
   .Ro0   (Ro0),    .Ro1   (Ro1),    .Ro2   (Ro2),    .Ro3   (Ro3),
   .Co0   (Co0),    .Co1   (Co1),    .Co2   (Co2),    .Co3   (Co3),
   .Lo0_fb(Lo0_fb), .Lo1_fb(Lo1_fb), .Lo2_fb(Lo2_fb), .Lo3_fb(Lo3_fb),
   .Ro0_fb(Ro0_fb), .Ro1_fb(Ro1_fb), .Ro2_fb(Ro2_fb), .Ro3_fb(Ro3_fb),
   .Co0_fb(Co0_fb), .Co1_fb(Co1_fb), .Co2_fb(Co2_fb), .Co3_fb(Co3_fb),

   .Li0   (Li0),    .Li1   (Li1),    .Li2   (Li2),    .Li3   (Li3),
   .Ri0   (Ri0),    .Ri1   (Ri1),    .Ri2   (Ri2),    .Ri3   (Ri3),
   .Ci0   (Ci0),    .Ci1   (Ci1),    .Ci2   (Ci2),    .Ci3   (Ci3),
   .Li0_fb(Li0_fb), .Li1_fb(Li1_fb), .Li2_fb(Li2_fb), .Li3_fb(Li3_fb),
   .Ri0_fb(Ri0_fb), .Ri1_fb(Ri1_fb), .Ri2_fb(Ri2_fb), .Ri3_fb(Ri3_fb),
   .Ci0_fb(Ci0_fb), .Ci1_fb(Ci1_fb), .Ci2_fb(Ci2_fb), .Ci3_fb(Ci3_fb),

   .ID(ID),     // 0 = This module is the NORTH Hub, 1 = This module is the SOUTH hub
   .reset(reset),
   .clock(clock)
   );


initial begin
   clock = 0;
   forever
      clock = #10 ~clock;
   end

initial begin
   ID       = 1'b0;
   reset    = 0;
   Li0      = 96'h0;    Li1   = 96'h0;      Li2 = 96'h0;      Li3 = 96'h0;
   Ri0      = 96'h0;    Ri1   = 96'h0;      Ri2 = 96'h0;      Ri3 = 96'h0;
   Ci0      = 96'h0;    Ci1   = 96'h0;      Ci2 = 96'h0;      Ci3 = 96'h0;
   Li0_push = 1'b0;  Li1_push = 1'b0;  Li2_push = 1'b0;  Li3_push = 1'b0;
   Ri0_push = 1'b0;  Ri1_push = 1'b0;  Ri2_push = 1'b0;  Ri3_push = 1'b0;
   Ci0_push = 1'b0;  Ci1_push = 1'b0;  Ci2_push = 1'b0;  Ci3_push = 1'b0;
   Lo0_irdy = 1'b0;  Lo1_irdy = 1'b0;  Lo2_irdy = 1'b0;  Lo3_irdy = 1'b0;
   Ro0_irdy = 1'b0;  Ro1_irdy = 1'b0;  Ro2_irdy = 1'b0;  Ro3_irdy = 1'b0;
   Co0_irdy = 1'b0;  Co1_irdy = 1'b0;  Co2_irdy = 1'b0;  Co3_irdy = 1'b0;

   repeat ( 10) @(posedge clock) #1;
   reset    = 1;
   repeat ( 10) @(posedge clock) #1;
   reset    = 0;
   repeat ( 10) @(posedge clock) #1;
   Li0      = 96'h0;    Li1   = 96'h1;      Li2 = 96'h2;      Li3 = 96'h3;
   Ri0      = 96'h4;    Ri1   = 96'h5;      Ri2 = 96'h6;      Ri3 = 96'h7;
   Ci0      = 96'h8;    Ci1   = 96'h9;      Ci2 = 96'ha;      Ci3 = 96'hb;
   Li0_push = 1'b1;  Li1_push = 1'b1;  Li2_push = 1'b1;  Li3_push = 1'b1;
   Ri0_push = 1'b1;  Ri1_push = 1'b1;  Ri2_push = 1'b1;  Ri3_push = 1'b1;
   Ci0_push = 1'b1;  Ci1_push = 1'b1;  Ci2_push = 1'b1;  Ci3_push = 1'b1;
   repeat (  1) @(posedge clock) #1;
   Li0      = 96'h10;   Li1   = 96'h11;     Li2 = 96'h12;     Li3 = 96'h13;
   Ri0      = 96'h14;   Ri1   = 96'h15;     Ri2 = 96'h16;     Ri3 = 96'h17;
   Ci0      = 96'h18;   Ci1   = 96'h19;     Ci2 = 96'h1a;     Ci3 = 96'h1b;
   repeat (  1) @(posedge clock) #1;
   Li0_push = 1'b0;  Li1_push = 1'b0;  Li2_push = 1'b0;  Li3_push = 1'b0;
   Ri0_push = 1'b0;  Ri1_push = 1'b0;  Ri2_push = 1'b0;  Ri3_push = 1'b0;
   Ci0_push = 1'b0;  Ci1_push = 1'b0;  Ci2_push = 1'b0;  Ci3_push = 1'b0;
   repeat (  5) @(posedge clock) #1;
   Lo0_irdy = 1'b1;  Lo1_irdy = 1'b1;  Lo2_irdy = 1'b1;  Lo3_irdy = 1'b1;
   Ro0_irdy = 1'b1;  Ro1_irdy = 1'b1;  Ro2_irdy = 1'b1;  Ro3_irdy = 1'b1;
   Co0_irdy = 1'b1;  Co1_irdy = 1'b1;  Co2_irdy = 1'b1;  Co3_irdy = 1'b1;
   repeat (  3) @(posedge clock) #1;
   Lo0_irdy = 1'b0;  Lo1_irdy = 1'b0;  Lo2_irdy = 1'b0;  Lo3_irdy = 1'b0;
   Ro0_irdy = 1'b0;  Ro1_irdy = 1'b0;  Ro2_irdy = 1'b0;  Ro3_irdy = 1'b0;
   Co0_irdy = 1'b0;  Co1_irdy = 1'b0;  Co2_irdy = 1'b0;  Co3_irdy = 1'b0;
   repeat (  2) @(posedge clock) #1;
   Lo0_irdy = 1'b1;  Lo1_irdy = 1'b1;  Lo2_irdy = 1'b1;  Lo3_irdy = 1'b1;
   Ro0_irdy = 1'b1;  Ro1_irdy = 1'b1;  Ro2_irdy = 1'b1;  Ro3_irdy = 1'b1;
   Co0_irdy = 1'b1;  Co1_irdy = 1'b1;  Co2_irdy = 1'b1;  Co3_irdy = 1'b1;
   repeat (  3) @(posedge clock) #1;
   Lo0_irdy = 1'b0;  Lo1_irdy = 1'b0;  Lo2_irdy = 1'b0;  Lo3_irdy = 1'b0;
   Ro0_irdy = 1'b0;  Ro1_irdy = 1'b0;  Ro2_irdy = 1'b0;  Ro3_irdy = 1'b0;
   Co0_irdy = 1'b0;  Co1_irdy = 1'b0;  Co2_irdy = 1'b0;  Co3_irdy = 1'b0;

   repeat (100) @(posedge clock) #1;
   $finish;
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