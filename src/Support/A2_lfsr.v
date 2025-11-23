/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//
//   Copyright © 2009 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         :   Library
//
//   Description          :   LFSR using data from Xilinx App Note
//                            http://www.xilinx.com/support/documentation/application_notes/xapp052.pdf
//                   moved to https://docs.amd.com/v/u/en-US/xapp052
//
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//
//   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//
//    n   XNOR                   n   XNOR                 n   XNOR                  n    XNOR
//    3 : 3,2                    45: 45,44,42,41          87: 87,74                 129: 129,124
//    4 : 4,3                    46: 46,45,26,25          88: 88,87,17,16           130: 130,127
//    5 : 5,3                    47: 47,42                89: 89,51                 131: 131,130,84,83
//    6 : 6,5                    48: 48,47,21,20          90: 90,89,72,71           132: 132,103
//    7 : 7,6                    49: 49,40                91: 91,90,8,7             133: 133,132,82,81
//    8 : 8,6,5,4                50: 50,49,24,23          92: 92,91,80,79           134: 134,77
//    9 : 9,5                    51: 51,50,36,35          93: 93,91                 135: 135,124
//    10: 10,7                   52: 52,49                94: 94,73                 136: 136,135,11,10
//    11: 11,9                   53: 53,52,38,37          95: 95,84                 137: 137,116
//    12: 12,6,4,1               54: 54,53,18,17          96: 96,94,49,47           138: 138,137,131,130
//    13: 13,4,3,1               55: 55,31                97: 97,91                 139: 139,136,134,131
//    14: 14,5,3,1               56: 56,55,35,34          98: 98,87                 140: 140,111
//    15: 15,14                  57: 57,50                99: 99,97,54,52           141: 141,140,110,109
//    16: 16,15,13,4             58: 58,39               100: 100,63                142: 142,121
//    17: 17,14                  59: 59,58,38,37         101: 101,100,95,94         143: 143,142,123,122
//    18: 18,11                  60: 60,59               102: 102,101,36,35         144: 144,143,75,74
//    19: 19,6,2,1               61: 61,60,46,45         103: 103,94                145: 145,93
//    20: 20,17                  62: 62,61,6,5           104: 104,103,94,93         146: 146,145,87,86
//    21: 21,19                  63: 63,62               105: 105,89                147: 147,146,110,109
//    22: 22,21                  64: 64,63,61,60         106: 106,91                148: 148,121
//    23: 23,18                  65: 65,47               107: 107,105,44,42         149: 149,148,40,39
//    24: 24,23,22,17            66: 66,65,57,56         108: 108,77                150: 150,97
//    25: 25,22                  67: 67,66,58,57         109: 109,108,103,102       151: 151,148
//    26: 26,6,2,1               68: 68,59               110: 110,109,98,97         152: 152,151,87,86
//    27: 27,5,2,1               69: 69,67,42,40         111: 111,101               153: 153,152
//    28: 28,25                  70: 70,69,55,54         112: 112,110,69,67         154: 154,152,27,25
//    29: 29,27                  71: 71,65               113: 113,104               155: 155,154,124,123
//    30: 30,6,4,1               72: 72,66,25,19         114: 114,113,33,32         156: 156,155,41,40
//    31: 31,28                  73: 73,48               115: 115,114,101,100       157: 157,156,131,130
//    32: 32,22,2,1              74: 74,73,59,58         116: 116,115,46,45         158: 158,157,132,131
//    33: 33,20                  75: 75,74,65,64         117: 117,115,99,97         159: 159,128
//    34: 34,27,2,1              76: 76,75,41,40         118: 118,85                160: 160,159,142,141
//    35: 35,33                  77: 77,76,47,46         119: 119,111               161: 161,143
//    36: 36,25                  78: 78,77,59,58         120: 120,113,9,2           162: 162,161,75,74
//    37: 37,5,4,3,2,1           79: 79,70               121: 121,103               163: 163,162,104,103
//    38: 38,6,5,1               80: 80,79,43,42         122: 122,121,63,62         164: 164,163,151,150
//    39: 39,35                  81: 81,77               123: 123,121               165: 165,164,135,134
//    40: 40,38,21,19            82: 82,79,47,44         124: 124,87                166: 166,165,128,127
//    41: 41,38                  83: 83,82,38,37         125: 125,124,18,17         167: 167,161
//    42: 42,41,20,19            84: 84,71               126: 126,125,90,89         168: 168,166,153,151
//    43: 43,42,38,37            85: 85,84,58,57         127: 127,126
//    44: 44,43,18,17            86: 86,85,74,73         128: 128,126,101,99
//
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

module A2_lfsr #(
   parameter LENGTH  = 32,          // Max length supported is 64.  may be extended to 168 using the above!
   parameter SEED    = {LENGTH{1'b0}}
   )(
   output [LENGTH-1:0]  LFSRo,
   input                enable,
   input                reset,
   input                clock
   );

reg   [LENGTH:1]  S;
wire              fb;

always @(posedge clock)
   if (reset)
      S  <= #1 SEED;
   else if (enable)
      if (S=={LENGTH{1'b1}})           // Detect lock-out condition
         S <= #1 {LENGTH{1'b0}};
      else
         S <= #1 {S[LENGTH-1:1],fb};

generate case(LENGTH)
   3 : assign fb = ~^{S[3],S[2]};
   4 : assign fb = ~^{S[4],S[3]};
   5 : assign fb = ~^{S[5],S[3]};
   6 : assign fb = ~^{S[6],S[5]};
   7 : assign fb = ~^{S[7],S[6]};
   8 : assign fb = ~^{S[8],S[6],S[5],S[4]};
   9 : assign fb = ~^{S[9],S[5]};
   10: assign fb = ~^{S[10],S[7]};
   11: assign fb = ~^{S[11],S[9]};
   12: assign fb = ~^{S[12],S[6],S[4],S[1]};
   13: assign fb = ~^{S[13],S[4],S[3],S[1]};
   14: assign fb = ~^{S[14],S[5],S[3],S[1]};
   15: assign fb = ~^{S[15],S[14]};
   16: assign fb = ~^{S[16],S[15],S[13],S[4]};
   17: assign fb = ~^{S[17],S[14]};
   18: assign fb = ~^{S[18],S[11]};
   19: assign fb = ~^{S[19],S[6],S[2],S[1]};
   20: assign fb = ~^{S[20],S[17]};
   21: assign fb = ~^{S[21],S[19]};
   22: assign fb = ~^{S[22],S[21]};
   23: assign fb = ~^{S[23],S[18]};
   24: assign fb = ~^{S[24],S[23],S[22],S[17]};
   25: assign fb = ~^{S[25],S[22]};
   26: assign fb = ~^{S[26],S[6],S[2],S[1]};
   27: assign fb = ~^{S[27],S[5],S[2],S[1]};
   28: assign fb = ~^{S[28],S[25]};
   29: assign fb = ~^{S[29],S[27]};
   30: assign fb = ~^{S[30],S[6],S[4],S[1]};
   31: assign fb = ~^{S[31],S[28]};
   32: assign fb = ~^{S[32],S[22],S[2],S[1]};
   33: assign fb = ~^{S[33],S[20]};
   34: assign fb = ~^{S[34],S[27],S[2],S[1]};
   35: assign fb = ~^{S[35],S[33]};
   36: assign fb = ~^{S[36],S[25]};
   37: assign fb = ~^{S[37],S[5],S[4],S[3],S[2],S[1]};
   38: assign fb = ~^{S[38],S[6],S[5],S[1]};
   39: assign fb = ~^{S[39],S[35]};
   40: assign fb = ~^{S[40],S[38],S[21],S[19]};
   41: assign fb = ~^{S[41],S[38]};
   42: assign fb = ~^{S[42],S[41],S[20],S[19]};
   43: assign fb = ~^{S[43],S[42],S[38],S[37]};
   44: assign fb = ~^{S[44],S[43],S[18],S[17]};
   45: assign fb = ~^{S[45],S[44],S[42],S[41]};
   46: assign fb = ~^{S[46],S[45],S[26],S[25]};
   47: assign fb = ~^{S[47],S[42]};
   48: assign fb = ~^{S[48],S[47],S[21],S[20]};
   49: assign fb = ~^{S[49],S[40]};
   50: assign fb = ~^{S[50],S[49],S[24],S[23]};
   51: assign fb = ~^{S[51],S[50],S[36],S[35]};
   52: assign fb = ~^{S[52],S[49]};
   53: assign fb = ~^{S[53],S[52],S[38],S[37]};
   54: assign fb = ~^{S[54],S[53],S[18],S[17]};
   55: assign fb = ~^{S[55],S[31]};
   56: assign fb = ~^{S[56],S[55],S[35],S[34]};
   57: assign fb = ~^{S[57],S[50]};
   58: assign fb = ~^{S[58],S[39]};
   59: assign fb = ~^{S[59],S[58],S[38],S[37]};
   60: assign fb = ~^{S[60],S[59]};
   61: assign fb = ~^{S[61],S[60],S[46],S[45]};
   62: assign fb = ~^{S[62],S[61],S[6], S[5] };
   63: assign fb = ~^{S[63],S[62]};
   endcase
endgenerate

assign   LFSRo = S;

endmodule
