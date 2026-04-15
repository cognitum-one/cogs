`timescale 1ns / 1ps    //Joel 1_31_24
//==============================================================================================================================
//
//   Copyright © 1996..2022 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : 3 way Clocked Square Sparrow Arbiter
//
//===============================================================================================================================
//      THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//===============================================================================================================================

`ifdef  synthesis //JLPL
   `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_a0/src/include/A2_project_settings.vh"
`else
   `include "A2_project_settings.vh"
`endif

module A2_arb3 (
   output wire [2:0] grant,
   input  wire [2:0] request,
   input  wire       advance,
   input  wire       reset,
   input  wire       clock
   );

reg  [2:0]  last;
reg  [2:0]  win;
reg         New;

reg  [4:0]  caseR;

always @* casez({last[2:0],request[2:0]})
   6'b001001  :  begin {New,win[2:0]} = 4'b1001; caseR = 0   ; end     //Joel Fix Case Values
   6'b001010  :  begin {New,win[2:0]} = 4'b1010; caseR = 1   ; end     //Joel Fix Case Values
   6'b001011  :  begin {New,win[2:0]} = 4'b1010; caseR = 2   ; end     //Joel Fix Case Values
   6'b001100  :  begin {New,win[2:0]} = 4'b1100; caseR = 3   ; end     //Joel Fix Case Values
   6'b001101  :  begin {New,win[2:0]} = 4'b1100; caseR = 4   ; end     //Joel Fix Case Values
   6'b001110  :  begin {New,win[2:0]} = 4'b1010; caseR = 5   ; end     //Joel Fix Case Values
   6'b001111  :  begin {New,win[2:0]} = 4'b1010; caseR = 6   ; end     //Joel Fix Case Values

   6'b010001  :  begin {New,win[2:0]} = 4'b1001; caseR = 7   ; end     //Joel Fix Case Values
   6'b010010  :  begin {New,win[2:0]} = 4'b1010; caseR = 8   ; end     //Joel Fix Case Values
   6'b010011  :  begin {New,win[2:0]} = 4'b1001; caseR = 9   ; end     //Joel Fix Case Values
   6'b010100  :  begin {New,win[2:0]} = 4'b1100; caseR = 10  ; end     //Joel Fix Case Values
   6'b010101  :  begin {New,win[2:0]} = 4'b1100; caseR = 11  ; end     //Joel Fix Case Values
   6'b010110  :  begin {New,win[2:0]} = 4'b1100; caseR = 12  ; end     //Joel Fix Case Values
   6'b010111  :  begin {New,win[2:0]} = 4'b1100; caseR = 13  ; end     //Joel Fix Case Values

   6'b100001  :  begin {New,win[2:0]} = 4'b1001; caseR = 14  ; end     //Joel Fix Case Values
   6'b100010  :  begin {New,win[2:0]} = 4'b1010; caseR = 15  ; end     //Joel Fix Case Values
   6'b100011  :  begin {New,win[2:0]} = 4'b1001; caseR = 16  ; end     //Joel Fix Case Values 
   6'b100100  :  begin {New,win[2:0]} = 4'b1100; caseR = 17  ; end     //Joel Fix Case Values 
   6'b100101  :  begin {New,win[2:0]} = 4'b1001; caseR = 18  ; end     //Joel Fix Case Values
   6'b100110  :  begin {New,win[2:0]} = 4'b1010; caseR = 19  ; end     //Joel Fix Case Values
   6'b100111  :  begin {New,win[2:0]} = 4'b1001; caseR = 20  ; end     //Joel Fix Case Values

   default         begin {New,win[2:0]} = 4'b0_000; caseR = 21  ; end
   endcase

always @(posedge clock)
   if (reset)              last <= `tCQ 3'b100;
   else if (New & advance) last <= `tCQ win[2:0];

assign   grant[2:0] =  win[2:0];
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
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
